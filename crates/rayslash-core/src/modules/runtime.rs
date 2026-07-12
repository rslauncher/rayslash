use std::{
    collections::BTreeMap,
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Mutex, OnceLock, mpsc},
    thread,
    time::Duration,
};

use crate::{
    APP_NAME,
    search::{ModuleAction, SearchResult, SearchResultIcon, SearchResultKind},
};
use serde::{Deserialize, Serialize};

use super::{
    ModulePackageManifest, ModulesConfig, PackageKind, installed_revocation, load_cached_registry,
    load_installed_modules,
};

const HOST_PROTOCOL: u32 = 1;
const HOST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_HOST_OUTPUT: u64 = 2 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct ModuleQueryBatch {
    pub results: Vec<SearchResult>,
    pub exclusive: bool,
    pub errors: Vec<String>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HostRequest<'a> {
    Handshake {
        protocol: u32,
    },
    Query {
        id: u64,
        query: &'a str,
        max_results: u32,
        locale: Option<&'a str>,
        settings_json: &'a str,
    },
}

#[derive(Deserialize)]
struct HostResponse {
    #[serde(rename = "type")]
    kind: String,
    id: Option<u64>,
    value: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct QueryValue {
    results: Vec<ResultValue>,
    exclusive: bool,
}

#[derive(Deserialize)]
struct ResultValue {
    id: String,
    title: String,
    subtitle: String,
    icon: IconValue,
    score: Option<u32>,
    action: ActionValue,
}

#[derive(Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum IconValue {
    PackagePath(String),
    Text(String),
    None,
}

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum ActionValue {
    CopyText(String),
    OpenUrl(String),
    OpenPath(String),
    ShowMessage(String),
    Notify((String, String)),
    RunApprovedCommand(Vec<String>),
    ScheduleNotification((u64, String, String)),
    ScheduleCommand((u64, Vec<String>)),
    None,
}

struct HostJob {
    query: String,
    max_results: usize,
    settings_json: String,
    response: mpsc::Sender<Result<ModuleQueryBatch, String>>,
}

struct HostProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    handshake: Vec<u8>,
}

static HOST_POOL: OnceLock<Mutex<BTreeMap<String, mpsc::Sender<HostJob>>>> = OnceLock::new();

pub fn query_installed_modules(
    query: &str,
    max_results: usize,
    config: &ModulesConfig,
    settings: &BTreeMap<String, String>,
) -> ModuleQueryBatch {
    let mut batch = ModuleQueryBatch::default();
    let Ok(installed) = load_installed_modules() else {
        return batch;
    };
    let revocations = load_cached_registry()
        .ok()
        .map(|registry| registry.revocations);
    let mut candidates = Vec::new();
    for (module_id, installed) in installed.modules {
        if !config.is_enabled(&module_id).unwrap_or(installed.enabled) {
            continue;
        }
        if let Some(revocation) = revocations.as_ref().and_then(|revocations| {
            installed_revocation(
                revocations,
                &module_id,
                &installed.version,
                &installed.digest,
            )
        }) {
            batch.errors.push(format!(
                "{module_id}: installed version was revoked: {}",
                revocation.reason
            ));
            continue;
        }
        let manifest_path = installed.install_path.join("module.toml");
        let manifest = match fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|text| toml::from_str::<ModulePackageManifest>(&text).ok())
        {
            Some(manifest) if manifest.id == module_id => manifest,
            _ => {
                batch
                    .errors
                    .push(format!("{module_id}: invalid installed manifest"));
                continue;
            }
        };
        if manifest.kind != PackageKind::Wasm {
            batch
                .errors
                .push(format!("{module_id}: declarative runtime is unavailable"));
            continue;
        }
        let settings_json = settings
            .get(&module_id)
            .cloned()
            .unwrap_or_else(|| "{}".into());
        candidates.push((module_id, installed.install_path, manifest, settings_json));
    }
    let responses = thread::scope(|scope| {
        candidates
            .into_iter()
            .map(|(module_id, install_path, manifest, settings_json)| {
                scope.spawn(move || {
                    let response = query_wasm_module(
                        &module_id,
                        &install_path,
                        &manifest,
                        query,
                        max_results,
                        &settings_json,
                    );
                    (module_id, response)
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .collect::<Vec<_>>()
    });
    let mut regular = Vec::new();
    let mut exclusive = Vec::new();
    for (module_id, response) in responses {
        match response {
            Ok(response) if response.exclusive => {
                batch.exclusive = true;
                exclusive.extend(response.results);
            }
            Ok(response) => regular.extend(response.results),
            Err(error) => batch.errors.push(format!("{module_id}: {error}")),
        }
    }
    batch.results = if batch.exclusive { exclusive } else { regular };
    batch.results.truncate(max_results);
    batch
}

fn query_wasm_module(
    module_id: &str,
    install_path: &Path,
    manifest: &ModulePackageManifest,
    query: &str,
    max_results: usize,
    settings_json: &str,
) -> Result<ModuleQueryBatch, String> {
    let key = format!("{module_id}:{}", install_path.display());
    let sender = host_sender(&key, module_id, install_path, manifest);
    let (response, receiver) = mpsc::channel();
    if sender
        .send(HostJob {
            query: query.to_owned(),
            max_results,
            settings_json: settings_json.to_owned(),
            response,
        })
        .is_err()
    {
        remove_host(&key);
        return Err("module host stopped unexpectedly".into());
    }
    match receiver.recv_timeout(HOST_TIMEOUT) {
        Ok(response) => response,
        Err(_) => {
            remove_host(&key);
            Err("module query timed out".into())
        }
    }
}

pub(super) fn probe_wasm_module(
    module_id: &str,
    install_path: &Path,
    manifest: &ModulePackageManifest,
) -> Result<(), String> {
    query_wasm_module(module_id, install_path, manifest, "", 1, "{}").map(|_| ())
}

fn host_pool() -> &'static Mutex<BTreeMap<String, mpsc::Sender<HostJob>>> {
    HOST_POOL.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn remove_host(key: &str) {
    host_pool()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(key);
}

fn host_sender(
    key: &str,
    module_id: &str,
    install_path: &Path,
    manifest: &ModulePackageManifest,
) -> mpsc::Sender<HostJob> {
    let mut pool = host_pool()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(sender) = pool.get(key) {
        return sender.clone();
    }
    let (sender, receiver) = mpsc::channel();
    let module_id = module_id.to_owned();
    let install_path = install_path.to_owned();
    let permissions = manifest.permissions.clone();
    thread::spawn(move || host_worker(module_id, install_path, permissions, receiver));
    pool.insert(key.to_owned(), sender.clone());
    sender
}

fn host_worker(
    module_id: String,
    install_path: PathBuf,
    permissions: super::PackagePermissions,
    receiver: mpsc::Receiver<HostJob>,
) {
    let mut process = HostProcess::start(&module_id, &install_path, &permissions);
    for job in receiver {
        let response = match &mut process {
            Ok(process) => process.query(
                &module_id,
                &install_path,
                &permissions,
                &job.query,
                job.max_results,
                &job.settings_json,
            ),
            Err(error) => Err(error.clone()),
        };
        let failed = response.is_err();
        let _ = job.response.send(response);
        if failed && process.is_ok() {
            break;
        }
    }
}

impl HostProcess {
    fn start(
        module_id: &str,
        install_path: &Path,
        permissions: &super::PackagePermissions,
    ) -> Result<Self, String> {
        let mut command = Command::new(module_host_path());
        command
            .arg("--module")
            .arg(install_path.join("module.wasm"))
            .arg("--cache-dir")
            .arg(module_cache_dir(module_id))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for origin in &permissions.network {
            command.arg("--network-origin").arg(origin);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("module host is not installed or could not start: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or("module host stdin is unavailable")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("module host stdout is unavailable")?;
        let mut process = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            handshake: Vec::new(),
        };
        write_request(
            &mut process.stdin,
            &HostRequest::Handshake {
                protocol: HOST_PROTOCOL,
            },
        )?;
        process.handshake = read_limited_line(&mut process.stdout)?;
        Ok(process)
    }

    fn query(
        &mut self,
        module_id: &str,
        install_path: &Path,
        permissions: &super::PackagePermissions,
        query: &str,
        max_results: usize,
        settings_json: &str,
    ) -> Result<ModuleQueryBatch, String> {
        write_request(
            &mut self.stdin,
            &HostRequest::Query {
                id: 1,
                query,
                max_results: u32::try_from(max_results.min(100)).unwrap_or(100),
                locale: None,
                settings_json,
            },
        )?;
        let response = read_limited_line(&mut self.stdout)?;
        let mut output = self.handshake.clone();
        output.extend_from_slice(&response);
        parse_host_output(module_id, install_path, permissions, settings_json, &output)
    }
}

impl Drop for HostProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_limited_line(reader: &mut impl BufRead) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    loop {
        let buffer = reader.fill_buf().map_err(|error| error.to_string())?;
        if buffer.is_empty() {
            return Err("module host closed its output".into());
        }
        let length = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(buffer.len(), |index| index + 1);
        if output.len().saturating_add(length) as u64 > MAX_HOST_OUTPUT {
            return Err("module host output exceeded its limit".into());
        }
        output.extend_from_slice(&buffer[..length]);
        reader.consume(length);
        if output.ends_with(b"\n") {
            return Ok(output);
        }
    }
}

fn write_request(writer: &mut impl Write, request: &HostRequest<'_>) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, request).map_err(|error| error.to_string())?;
    writer.write_all(b"\n").map_err(|error| error.to_string())
}

fn parse_host_output(
    module_id: &str,
    install_path: &Path,
    permissions: &super::PackagePermissions,
    settings_json: &str,
    output: &[u8],
) -> Result<ModuleQueryBatch, String> {
    let text = std::str::from_utf8(output).map_err(|_| "module host output is not UTF-8")?;
    let mut lines = text.lines();
    let handshake: HostResponse = serde_json::from_str(lines.next().ok_or("missing handshake")?)
        .map_err(|error| error.to_string())?;
    if handshake.kind != "handshake" || handshake.value != Some(HOST_PROTOCOL.into()) {
        return Err("module host protocol mismatch".into());
    }
    let response: HostResponse =
        serde_json::from_str(lines.next().ok_or("missing query response")?)
            .map_err(|error| error.to_string())?;
    if response.kind != "query" || response.id != Some(1) {
        return Err("invalid module query response".into());
    }
    if let Some(error) = response.error {
        return Err(error);
    }
    let value: QueryValue = serde_json::from_value(response.value.ok_or("missing query value")?)
        .map_err(|error| error.to_string())?;
    let results = value
        .results
        .into_iter()
        .map(|result| SearchResult {
            title: result.title,
            flair: String::new(),
            subtitle: result.subtitle,
            icon: map_icon(result.icon, install_path),
            kind: SearchResultKind::Module {
                module_id: module_id.to_owned(),
                result_id: result.id,
                action: map_action(result.action, permissions, settings_json),
                score: result.score,
            },
        })
        .collect();
    Ok(ModuleQueryBatch {
        results,
        exclusive: value.exclusive,
        errors: Vec::new(),
    })
}

fn map_icon(icon: IconValue, install_path: &Path) -> SearchResultIcon {
    match icon {
        IconValue::PackagePath(path) => {
            let relative = Path::new(&path);
            if relative
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
            {
                SearchResultIcon::Module {
                    label: String::new(),
                    path: Some(install_path.join(relative)),
                }
            } else {
                SearchResultIcon::Module {
                    label: String::new(),
                    path: None,
                }
            }
        }
        IconValue::Text(label) => SearchResultIcon::Module { label, path: None },
        IconValue::None => SearchResultIcon::Module {
            label: String::new(),
            path: None,
        },
    }
}

fn map_action(
    action: ActionValue,
    permissions: &super::PackagePermissions,
    settings_json: &str,
) -> ModuleAction {
    match action {
        ActionValue::CopyText(value) => ModuleAction::CopyText(value),
        ActionValue::OpenUrl(value) => ModuleAction::OpenUrl(value),
        ActionValue::OpenPath(value) if settings_contains(settings_json, &value) => {
            ModuleAction::OpenPath(value.into())
        }
        ActionValue::OpenPath(_) => ModuleAction::ShowMessage(
            "Module path was not present in its approved settings.".into(),
        ),
        ActionValue::ShowMessage(value) => ModuleAction::ShowMessage(value),
        ActionValue::Notify((title, body)) if permissions.notifications => {
            ModuleAction::Notify { title, body }
        }
        ActionValue::RunApprovedCommand(value) if permissions.commands => {
            ModuleAction::RunApprovedCommand(value)
        }
        ActionValue::ScheduleNotification((delay, title, body)) if permissions.notifications => {
            ModuleAction::ScheduleNotification { delay, title, body }
        }
        ActionValue::ScheduleCommand((delay, command)) if permissions.commands => {
            ModuleAction::ScheduleCommand { delay, command }
        }
        ActionValue::Notify(_) | ActionValue::ScheduleNotification(_) => {
            ModuleAction::ShowMessage("Module notification permission was not granted.".into())
        }
        ActionValue::RunApprovedCommand(_) | ActionValue::ScheduleCommand(_) => {
            ModuleAction::ShowMessage("Module command permission was not granted.".into())
        }
        ActionValue::None => ModuleAction::None,
    }
}

fn settings_contains(settings_json: &str, expected: &str) -> bool {
    fn contains(value: &serde_json::Value, expected: &str) -> bool {
        match value {
            serde_json::Value::String(value) => value == expected,
            serde_json::Value::Array(values) => {
                values.iter().any(|value| contains(value, expected))
            }
            serde_json::Value::Object(values) => {
                values.values().any(|value| contains(value, expected))
            }
            _ => false,
        }
    }
    serde_json::from_str(settings_json)
        .ok()
        .is_some_and(|value| contains(&value, expected))
}

fn module_host_path() -> PathBuf {
    env::var_os("RAYSLASH_MODULE_HOST")
        .map(PathBuf::from)
        .or_else(|| {
            [
                "/app/libexec/rayslash/rayslash-module-host",
                "/usr/libexec/rayslash/rayslash-module-host",
            ]
            .into_iter()
            .map(PathBuf::from)
            .find(|path| path.is_file())
        })
        .unwrap_or_else(|| PathBuf::from("rayslash-module-host"))
}

fn module_cache_dir(module_id: &str) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(env::temp_dir)
        .join(APP_NAME)
        .join("modules")
        .join(module_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typed_host_result() {
        let output = br#"{"type":"handshake","value":1}
{"type":"query","id":1,"value":{"results":[{"id":"one","title":"1","subtitle":"result","icon":{"type":"text","value":"="},"score":null,"action":{"type":"copy_text","value":"1"}}],"exclusive":true}}
"#;
        let parsed = parse_host_output(
            "example.module",
            Path::new("/tmp/module"),
            &super::super::PackagePermissions::default(),
            "{}",
            output,
        )
        .expect("valid host output");
        assert!(parsed.exclusive);
        assert_eq!(parsed.results.len(), 1);
    }

    #[test]
    fn path_actions_must_match_settings() {
        let allowed = map_action(
            ActionValue::OpenPath("/home/user/notes".into()),
            &super::super::PackagePermissions::default(),
            r#"{"target":"/home/user/notes"}"#,
        );
        assert!(matches!(allowed, ModuleAction::OpenPath(_)));
        let blocked = map_action(
            ActionValue::OpenPath("/etc/shadow".into()),
            &super::super::PackagePermissions::default(),
            r#"{"target":"/home/user/notes"}"#,
        );
        assert!(matches!(blocked, ModuleAction::ShowMessage(_)));
    }
}
