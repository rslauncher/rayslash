use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, BufRead, BufReader, Write},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Mutex, OnceLock, mpsc},
    thread,
    time::{Duration, Instant, SystemTime},
};

use crate::{
    APP_NAME,
    config::{AliasConfig, WebSearchConfig},
    providers::ProviderExecutionHint,
    search::{ModuleAction, SearchResult, SearchResultIcon, SearchResultKind},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    ModulePackageManifest, ModulesConfig, PackageKind, installed_revocation, load_cached_registry,
    load_installed_modules, registry::registry_cache_pointer_modified,
};

const HOST_PROTOCOL: u32 = 1;
const HOST_TIMEOUT: Duration = Duration::from_secs(5);
const NETWORK_HOST_TIMEOUT: Duration = Duration::from_secs(10);
const HOST_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_HOST_OUTPUT: u64 = 2 * 1024 * 1024;
const AOT_FORMAT: &str = "wasmtime44-component-fuel-v1";

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
    timeout: Duration,
    response: mpsc::Sender<Result<ModuleQueryBatch, String>>,
}

struct HostProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    handshake: Vec<u8>,
}

static HOST_POOL: OnceLock<Mutex<BTreeMap<String, mpsc::Sender<HostJob>>>> = OnceLock::new();
static RUNTIME_SNAPSHOT: OnceLock<Mutex<Option<RuntimeSnapshot>>> = OnceLock::new();

#[derive(Clone)]
struct RuntimeCandidate {
    module_id: String,
    install_path: PathBuf,
    manifest: ModulePackageManifest,
    installed_enabled: bool,
    settings_json: String,
}

struct RuntimeSnapshot {
    installed_path: Option<PathBuf>,
    installed_modified: Option<SystemTime>,
    registry_modified: Option<SystemTime>,
    candidates: Vec<RuntimeCandidate>,
    errors: Vec<String>,
}

pub fn installed_module_execution_hint(
    query: &str,
    config: &ModulesConfig,
    settings: &BTreeMap<String, String>,
) -> ProviderExecutionHint {
    let (candidates, _errors) = runtime_candidates(config, settings);
    if candidates
        .iter()
        .filter(|candidate| module_matches_query(candidate, query))
        .any(|candidate| !candidate.manifest.permissions.network.is_empty())
    {
        ProviderExecutionHint::DebouncedNetwork { debounce_ms: 150 }
    } else {
        ProviderExecutionHint::Local
    }
}

pub fn query_installed_modules(
    query: &str,
    max_results: usize,
    config: &ModulesConfig,
    settings: &BTreeMap<String, String>,
) -> ModuleQueryBatch {
    let mut batch = ModuleQueryBatch::default();
    let (candidates, errors) = runtime_candidates(config, settings);
    batch.errors = errors;
    let candidates = candidates
        .into_iter()
        .filter(|candidate| module_matches_query(candidate, query))
        .collect::<Vec<_>>();
    let responses = thread::scope(|scope| {
        candidates
            .into_iter()
            .map(|candidate| {
                scope.spawn(move || {
                    let response = query_wasm_module(
                        &candidate.module_id,
                        &candidate.install_path,
                        &candidate.manifest,
                        query,
                        max_results,
                        &candidate.settings_json,
                    );
                    (candidate.module_id, response)
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

fn runtime_candidates(
    config: &ModulesConfig,
    settings: &BTreeMap<String, String>,
) -> (Vec<RuntimeCandidate>, Vec<String>) {
    let installed_path = super::installed_modules_file();
    let installed_modified = installed_path
        .as_deref()
        .and_then(|path| fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok());
    let registry_modified = registry_cache_pointer_modified();
    let cache = RUNTIME_SNAPSHOT.get_or_init(|| Mutex::new(None));
    let mut cache = cache.lock().unwrap_or_else(|error| error.into_inner());
    let stale = runtime_snapshot_is_stale(
        cache.as_ref(),
        &installed_path,
        installed_modified,
        registry_modified,
    );
    if stale {
        *cache = Some(load_runtime_snapshot(
            installed_path.clone(),
            installed_modified,
            registry_modified,
        ));
    }
    let snapshot = cache.as_ref().expect("runtime snapshot was initialized");
    let candidates = snapshot
        .candidates
        .iter()
        .filter(|candidate| {
            config
                .is_enabled(&candidate.module_id)
                .unwrap_or(candidate.installed_enabled)
        })
        .cloned()
        .map(|mut candidate| {
            candidate.settings_json = settings
                .get(&candidate.module_id)
                .cloned()
                .unwrap_or_else(|| "{}".into());
            candidate
        })
        .collect();
    (candidates, snapshot.errors.clone())
}

fn runtime_snapshot_is_stale(
    snapshot: Option<&RuntimeSnapshot>,
    installed_path: &Option<PathBuf>,
    installed_modified: Option<SystemTime>,
    registry_modified: Option<SystemTime>,
) -> bool {
    snapshot.is_none_or(|snapshot| {
        &snapshot.installed_path != installed_path
            || snapshot.installed_modified != installed_modified
            || snapshot.registry_modified != registry_modified
    })
}

fn load_runtime_snapshot(
    installed_path: Option<PathBuf>,
    installed_modified: Option<SystemTime>,
    registry_modified: Option<SystemTime>,
) -> RuntimeSnapshot {
    let mut snapshot = RuntimeSnapshot {
        installed_path,
        installed_modified,
        registry_modified,
        candidates: Vec::new(),
        errors: Vec::new(),
    };
    let installed = match load_installed_modules() {
        Ok(installed) => installed,
        Err(error) => {
            snapshot
                .errors
                .push(format!("could not load installed module state: {error}"));
            return snapshot;
        }
    };
    let revocations = load_cached_registry()
        .ok()
        .map(|registry| registry.revocations);
    for (module_id, installed) in installed.modules {
        if let Some(revocation) = revocations.as_ref().and_then(|revocations| {
            installed_revocation(
                revocations,
                &module_id,
                &installed.version,
                &installed.digest,
            )
        }) {
            snapshot.errors.push(format!(
                "{module_id}: installed version was revoked: {}",
                revocation.reason
            ));
            continue;
        }
        let manifest_path = installed.install_path.join("module.toml");
        let mut manifest = match fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|text| toml::from_str::<ModulePackageManifest>(&text).ok())
        {
            Some(manifest) if manifest.id == module_id => manifest,
            _ => {
                snapshot
                    .errors
                    .push(format!("{module_id}: invalid installed manifest"));
                continue;
            }
        };
        if manifest.permissions != installed.permissions {
            snapshot.errors.push(format!(
                "{module_id}: installed manifest permissions no longer match verified state"
            ));
            continue;
        }
        manifest.permissions = installed.permissions;
        if manifest.kind != PackageKind::Wasm {
            snapshot
                .errors
                .push(format!("{module_id}: declarative runtime is unavailable"));
            continue;
        }
        snapshot.candidates.push(RuntimeCandidate {
            module_id,
            install_path: installed.install_path,
            manifest,
            installed_enabled: installed.enabled,
            settings_json: String::new(),
        });
    }
    snapshot
}

fn module_matches_query(candidate: &RuntimeCandidate, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return false;
    }
    match candidate.module_id.as_str() {
        super::CALCULATOR_MODULE_ID => calculation_hint(query),
        super::UNITS_MODULE_ID => conversion_hint(query),
        super::CURRENCY_MODULE_ID => currency_hint(query),
        super::TIME_MODULE_ID => starts_with_ascii(query, "time in "),
        super::TIMERS_MODULE_ID => timer_hint(query),
        super::WEB_SEARCH_MODULE_ID => web_search_hint(query, &candidate.settings_json),
        super::ALIASES_MODULE_ID => aliases_hint(query, &candidate.settings_json),
        _ => {
            let triggers = candidate
                .manifest
                .providers
                .iter()
                .flat_map(|provider| &provider.triggers)
                .collect::<Vec<_>>();
            triggers.is_empty()
                || triggers
                    .iter()
                    .any(|trigger| query_triggered_by(query, trigger))
        }
    }
}

fn calculation_hint(query: &str) -> bool {
    (query.chars().any(|ch| ch.is_ascii_digit())
        && query
            .chars()
            .any(|ch| matches!(ch, '+' | '-' | '*' | '/' | '^' | '=' | '(' | ')' | '.')))
        || ["sqrt(", "sin(", "cos("]
            .iter()
            .any(|prefix| query.to_ascii_lowercase().contains(prefix))
}

fn conversion_hint(query: &str) -> bool {
    let words = query.split_whitespace().collect::<Vec<_>>();
    match words.as_slice() {
        [amount, _, connector, _] => {
            amount.replace(',', "").parse::<f64>().is_ok()
                && matches!(connector.to_ascii_lowercase().as_str(), "to" | "in")
        }
        [amount_and_unit, connector, _] => {
            compact_conversion_amount(amount_and_unit)
                && matches!(connector.to_ascii_lowercase().as_str(), "to" | "in")
        }
        _ => false,
    }
}

fn compact_conversion_amount(value: &str) -> bool {
    value
        .char_indices()
        .rev()
        .filter(|(index, _)| *index > 0)
        .any(|(index, _)| {
            !value[index..].is_empty() && value[..index].replace(',', "").parse::<f64>().is_ok()
        })
}

fn currency_hint(query: &str) -> bool {
    let words = query.split_whitespace().collect::<Vec<_>>();
    match words.as_slice() {
        [amount, from, connector, to] => {
            amount.replace(',', "").parse::<f64>().is_ok()
                && connector.eq_ignore_ascii_case("to")
                && [*from, *to].iter().all(|word| currency_code(word))
        }
        [amount_and_from, connector, to] => {
            compact_currency_amount(amount_and_from)
                && connector.eq_ignore_ascii_case("to")
                && currency_code(to)
        }
        _ => false,
    }
}

fn compact_currency_amount(value: &str) -> bool {
    value.char_indices().any(|(index, _)| {
        index > 0
            && currency_code(&value[index..])
            && value[..index].replace(',', "").parse::<f64>().is_ok()
    })
}

fn currency_code(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn timer_hint(query: &str) -> bool {
    let timer_or_reminder = [
        "timer ",
        "reminder in ",
        "remind in ",
        "remind me in ",
        "remind me to ",
        "remind to ",
    ]
    .iter()
    .any(|prefix| starts_with_ascii(query, prefix));
    if timer_or_reminder {
        return true;
    }

    let query = query.trim().to_ascii_lowercase();
    query.len() >= 3
        && [
            "reboot",
            "restart",
            "shutdown",
            "shut down",
            "turn off",
            "poweroff",
            "logout",
            "log out",
            "lock",
        ]
        .iter()
        .any(|action| {
            action.starts_with(&query)
                || query == *action
                || query
                    .strip_prefix(action)
                    .is_some_and(|suffix| suffix.starts_with(" in "))
        })
}

fn web_search_hint(query: &str, settings_json: &str) -> bool {
    #[derive(Deserialize)]
    struct Settings {
        #[serde(default)]
        searches: Vec<WebSearchConfig>,
    }
    serde_json::from_str::<Settings>(settings_json)
        .ok()
        .is_some_and(|settings| {
            settings
                .searches
                .iter()
                .filter(|search| search.enabled)
                .any(|search| query_triggered_by(query, &search.keyword))
        })
}

fn aliases_hint(query: &str, settings_json: &str) -> bool {
    #[derive(Deserialize)]
    struct Settings {
        #[serde(default)]
        aliases: Vec<AliasConfig>,
    }
    let query = query.to_ascii_lowercase();
    serde_json::from_str::<Settings>(settings_json)
        .ok()
        .is_some_and(|settings| {
            settings.aliases.iter().any(|alias| {
                let name = alias.name.to_ascii_lowercase();
                let trigger = alias.query.to_ascii_lowercase();
                name.contains(&query) || trigger.contains(&query) || query.contains(&trigger)
            })
        })
}

fn query_triggered_by(query: &str, trigger: &str) -> bool {
    let query = query.trim();
    let trigger = trigger.trim();
    !trigger.is_empty()
        && query
            .get(..trigger.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(trigger))
        && query
            .get(trigger.len()..)
            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
}

fn starts_with_ascii(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
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
    let timeout = if manifest.permissions.network.is_empty() {
        HOST_TIMEOUT
    } else {
        NETWORK_HOST_TIMEOUT
    };
    let mut sender = host_sender(&key, module_id, install_path, manifest);
    let (mut response, mut receiver) = mpsc::channel();
    let mut job = HostJob {
        query: query.to_owned(),
        max_results,
        settings_json: settings_json.to_owned(),
        timeout,
        response,
    };
    if let Err(error) = sender.send(job) {
        remove_host(&key);
        sender = host_sender(&key, module_id, install_path, manifest);
        (response, receiver) = mpsc::channel();
        job = HostJob {
            query: error.0.query,
            max_results: error.0.max_results,
            settings_json: error.0.settings_json,
            timeout: error.0.timeout,
            response,
        };
        sender
            .send(job)
            .map_err(|_| "module host stopped unexpectedly".to_owned())?;
    }
    match receiver.recv_timeout(timeout + Duration::from_millis(250)) {
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

pub(super) fn stop_wasm_module(module_id: &str, install_path: &Path) {
    remove_host(&format!("{module_id}:{}", install_path.display()));
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
    while let Ok(job) = receiver.recv_timeout(HOST_IDLE_TIMEOUT) {
        let response = match &mut process {
            Ok(process) => process.query(&module_id, &install_path, &permissions, &job),
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
        match ensure_compiled_component(module_id, install_path) {
            Ok(compiled_component) => {
                match Self::start_artifact(module_id, permissions, &compiled_component) {
                    Ok(process) => Ok(process),
                    Err(first_error) => {
                        // A CPU migration, Wasmtime/configuration upgrade, or interrupted
                        // cache write can make a correctly named artifact incompatible.
                        let _ = fs::remove_file(&compiled_component);
                        let rebuilt = ensure_compiled_component(module_id, install_path)?;
                        Self::start_artifact(module_id, permissions, &rebuilt).map_err(|error| {
                            format!("{first_error}; rebuilt AOT artifact also failed: {error}")
                        })
                    }
                }
            }
            Err(compiler_error) => {
                // During a coordinated package rollout an older compiler-capable host can
                // still execute portable Wasm. The new runtime-only host rejects it, so a
                // package that omits both capabilities fails closed with both diagnostics.
                Self::start_artifact(module_id, permissions, &install_path.join("module.wasm"))
                    .map_err(|host_error| format!("{compiler_error}; {host_error}"))
            }
        }
    }

    fn start_artifact(
        module_id: &str,
        permissions: &super::PackagePermissions,
        compiled_component: &Path,
    ) -> Result<Self, String> {
        let mut command = Command::new(module_host_path());
        command
            .arg("--module")
            .arg(compiled_component)
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
        process.handshake = read_limited_line(&mut process.stdout, HOST_TIMEOUT)?;
        Ok(process)
    }

    fn query(
        &mut self,
        module_id: &str,
        install_path: &Path,
        permissions: &super::PackagePermissions,
        job: &HostJob,
    ) -> Result<ModuleQueryBatch, String> {
        write_request(
            &mut self.stdin,
            &HostRequest::Query {
                id: 1,
                query: &job.query,
                max_results: u32::try_from(job.max_results.min(100)).unwrap_or(100),
                locale: None,
                settings_json: &job.settings_json,
            },
        )?;
        let response = read_limited_line(&mut self.stdout, job.timeout)?;
        let mut output = self.handshake.clone();
        output.extend_from_slice(&response);
        parse_host_output(
            module_id,
            install_path,
            permissions,
            &job.settings_json,
            &output,
        )
    }
}

fn ensure_compiled_component(module_id: &str, install_path: &Path) -> Result<PathBuf, String> {
    let portable = install_path.join("module.wasm");
    let module = fs::read(&portable).map_err(|error| {
        format!(
            "failed to read module component {}: {error}",
            portable.display()
        )
    })?;
    let digest = format!("{:x}", Sha256::digest(&module));
    let directory = module_cache_dir(module_id).join("compiled");
    let compiled = directory.join(format!("{AOT_FORMAT}-{}.cwasm", &digest[..32]));
    if compiled.is_file() {
        let _ = fs::remove_dir_all(module_cache_dir(module_id).join("wasmtime"));
        return Ok(compiled);
    }
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "failed to create compiled module cache {}: {error}",
            directory.display()
        )
    })?;
    let output = Command::new(module_compiler_path())
        .arg("--module")
        .arg(&portable)
        .arg("--output")
        .arg(&compiled)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("module compiler is not installed or could not start: {error}"))?;
    if !output.status.success() || !compiled.is_file() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "module compilation failed{}",
            if detail.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", detail.trim())
            }
        ));
    }
    prune_compiled_components(&directory, &compiled);
    let _ = fs::remove_dir_all(module_cache_dir(module_id).join("wasmtime"));
    Ok(compiled)
}

fn prune_compiled_components(directory: &Path, keep: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut files = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path == keep || path.extension().and_then(|value| value.to_str()) != Some("cwasm") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _)| *modified);
    for (_, path) in files.into_iter().rev().skip(1) {
        let _ = fs::remove_file(path);
    }
}

impl Drop for HostProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_limited_line(
    reader: &mut BufReader<ChildStdout>,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let deadline = Instant::now() + timeout;
    loop {
        if reader.buffer().is_empty() {
            wait_for_pipe(reader.get_ref(), deadline)?;
        }
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

fn wait_for_pipe(pipe: &ChildStdout, deadline: Instant) -> Result<(), String> {
    #[repr(C)]
    struct PollFd {
        fd: std::os::raw::c_int,
        events: std::os::raw::c_short,
        revents: std::os::raw::c_short,
    }

    unsafe extern "C" {
        fn poll(fds: *mut PollFd, nfds: usize, timeout: std::os::raw::c_int)
        -> std::os::raw::c_int;
    }

    const POLLIN: std::os::raw::c_short = 0x0001;
    const POLLERR: std::os::raw::c_short = 0x0008;
    const POLLHUP: std::os::raw::c_short = 0x0010;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("module host response timed out".into());
        }
        let timeout_ms = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
        let mut descriptor = PollFd {
            fd: pipe.as_raw_fd(),
            events: POLLIN,
            revents: 0,
        };
        let result = unsafe { poll(&mut descriptor, 1, timeout_ms) };
        if result > 0 && descriptor.revents & (POLLIN | POLLERR | POLLHUP) != 0 {
            return Ok(());
        }
        if result == 0 {
            return Err("module host response timed out".into());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!("module host response poll failed: {error}"));
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
        .map(|result| {
            validate_result(&result)?;
            Ok(SearchResult {
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
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ModuleQueryBatch {
        results,
        exclusive: value.exclusive,
        errors: Vec::new(),
    })
}

fn validate_result(result: &ResultValue) -> Result<(), String> {
    validate_text("result ID", &result.id, 1, 256)?;
    validate_text("result title", &result.title, 1, 512)?;
    validate_text("result subtitle", &result.subtitle, 0, 1024)?;
    match &result.icon {
        IconValue::PackagePath(path) => {
            let path = Path::new(path);
            if path.as_os_str().len() > 512
                || path.is_absolute()
                || path
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err("invalid module package icon path".into());
            }
        }
        IconValue::Text(value) => validate_text("text icon", value, 1, 16)?,
        IconValue::None => {}
    }
    match &result.action {
        ActionValue::CopyText(value) => validate_text("copy action", value, 0, 64 * 1024)?,
        ActionValue::OpenUrl(value) => {
            let authority = value
                .strip_prefix("https://")
                .and_then(|rest| rest.split('/').next());
            if value.len() > 4096
                || value.chars().any(char::is_control)
                || authority.is_none_or(|authority| authority.is_empty() || authority.contains('@'))
            {
                return Err("invalid module URL action".into());
            }
        }
        ActionValue::OpenPath(value) => validate_text("path action", value, 1, 4096)?,
        ActionValue::ShowMessage(value) => validate_text("message action", value, 0, 4096)?,
        ActionValue::Notify((title, body)) => {
            validate_text("notification title", title, 1, 256)?;
            validate_text("notification body", body, 0, 4096)?;
        }
        ActionValue::RunApprovedCommand(command) => validate_command(command)?,
        ActionValue::ScheduleNotification((delay, title, body)) => {
            validate_delay(*delay)?;
            validate_text("notification title", title, 1, 256)?;
            validate_text("notification body", body, 0, 4096)?;
        }
        ActionValue::ScheduleCommand((delay, command)) => {
            validate_delay(*delay)?;
            validate_command(command)?;
        }
        ActionValue::None => {}
    }
    Ok(())
}

fn validate_text(label: &str, value: &str, minimum: usize, maximum: usize) -> Result<(), String> {
    let length = value.chars().count();
    if length < minimum || length > maximum || value.chars().any(char::is_control) {
        return Err(format!("invalid module {label}"));
    }
    Ok(())
}

fn validate_command(command: &[String]) -> Result<(), String> {
    if command.is_empty() || command.len() > 32 {
        return Err("invalid module command action".into());
    }
    for (index, argument) in command.iter().enumerate() {
        validate_text("command argument", argument, usize::from(index == 0), 4096)?;
    }
    Ok(())
}

fn validate_delay(delay: u64) -> Result<(), String> {
    (delay <= 31_536_000)
        .then_some(())
        .ok_or_else(|| "module scheduled action exceeds one year".into())
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
            let executable = env::current_exe().ok();
            let home = dirs::home_dir();
            module_host_candidates(executable.as_deref(), home.as_deref())
                .into_iter()
                .find(|path| path.is_file())
        })
        .unwrap_or_else(|| PathBuf::from("rayslash-module-host"))
}

fn module_compiler_path() -> PathBuf {
    env::var_os("RAYSLASH_MODULE_COMPILER")
        .map(PathBuf::from)
        .or_else(|| {
            let host = module_host_path();
            let sibling = host.with_file_name("rayslash-module-compiler");
            sibling.is_file().then_some(sibling)
        })
        .unwrap_or_else(|| PathBuf::from("rayslash-module-compiler"))
}

fn module_host_candidates(executable: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(binary_dir) = executable.and_then(Path::parent) {
        candidates.push(binary_dir.join("../libexec/rayslash/rayslash-module-host"));
    }
    candidates.push(PathBuf::from("/app/libexec/rayslash/rayslash-module-host"));
    if let Some(home) = home {
        candidates.push(home.join(".local/libexec/rayslash/rayslash-module-host"));
    }
    candidates.push(PathBuf::from(
        "/usr/local/libexec/rayslash/rayslash-module-host",
    ));
    candidates.push(PathBuf::from("/usr/libexec/rayslash/rayslash-module-host"));
    candidates
}

fn module_cache_dir(module_id: &str) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(env::temp_dir)
        .join(APP_NAME)
        .join("modules")
        .join(module_id)
}

#[cfg(test)]
mod routing_tests {
    use super::*;

    #[test]
    fn official_module_routing_avoids_unrelated_app_queries() {
        assert!(!calculation_hint("firefox"));
        assert!(!conversion_hint("visual studio code"));
        assert!(!currency_hint("open my documents"));
        assert!(!timer_hint("terminal"));
    }

    #[test]
    fn official_module_routing_recognizes_supported_query_shapes() {
        assert!(calculation_hint("999 * 42"));
        assert!(conversion_hint("10 km to mi"));
        assert!(conversion_hint("10f to c"));
        assert!(conversion_hint("-40fahrenheit to celsius"));
        assert!(currency_hint("25 BRL to USD"));
        assert!(currency_hint("10usd to brl"));
        assert!(currency_hint("1,250.50EUR to USD"));
        assert!(timer_hint("timer 5m tea"));
        assert!(timer_hint("remind me in 30 to feed the cat"));
        assert!(timer_hint("reb"));
        assert!(timer_hint("loc"));
        assert!(timer_hint("log"));
        assert!(timer_hint("turn off"));
        assert!(timer_hint("log out"));
        assert!(timer_hint("shut down"));
        assert!(starts_with_ascii("TIME IN Tokyo", "time in "));
    }

    #[test]
    fn trigger_routing_requires_terms_after_the_keyword() {
        assert!(query_triggered_by("search rust", "search"));
        assert!(!query_triggered_by("search", "search"));
        assert!(!query_triggered_by("research rust", "search"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_host_discovery_prefers_the_host_bundled_with_the_launcher() {
        let candidates = module_host_candidates(
            Some(Path::new("/home/user/.local/bin/rayslash")),
            Some(Path::new("/home/user")),
        );
        assert_eq!(
            candidates,
            [
                "/home/user/.local/bin/../libexec/rayslash/rayslash-module-host",
                "/app/libexec/rayslash/rayslash-module-host",
                "/home/user/.local/libexec/rayslash/rayslash-module-host",
                "/usr/local/libexec/rayslash/rayslash-module-host",
                "/usr/libexec/rayslash/rayslash-module-host",
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn runtime_snapshot_invalidates_for_installs_and_registry_changes() {
        let installed_path = Some(PathBuf::from("/tmp/installed.json"));
        let snapshot = RuntimeSnapshot {
            installed_path: installed_path.clone(),
            installed_modified: Some(SystemTime::UNIX_EPOCH),
            registry_modified: Some(SystemTime::UNIX_EPOCH),
            candidates: Vec::new(),
            errors: Vec::new(),
        };

        assert!(!runtime_snapshot_is_stale(
            Some(&snapshot),
            &installed_path,
            Some(SystemTime::UNIX_EPOCH),
            Some(SystemTime::UNIX_EPOCH),
        ));
        assert!(runtime_snapshot_is_stale(
            Some(&snapshot),
            &installed_path,
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
            Some(SystemTime::UNIX_EPOCH),
        ));
        assert!(runtime_snapshot_is_stale(
            Some(&snapshot),
            &installed_path,
            Some(SystemTime::UNIX_EPOCH),
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
        ));
    }

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

    #[test]
    fn launcher_rejects_hostile_result_fields_again() {
        let result = ResultValue {
            id: "one".into(),
            title: "bad\nresult".into(),
            subtitle: String::new(),
            icon: IconValue::None,
            score: None,
            action: ActionValue::None,
        };
        assert!(validate_result(&result).is_err());
        let command = ResultValue {
            id: "one".into(),
            title: "Result".into(),
            subtitle: String::new(),
            icon: IconValue::None,
            score: None,
            action: ActionValue::RunApprovedCommand(Vec::new()),
        };
        assert!(validate_result(&command).is_err());
    }

    #[test]
    fn silent_host_pipe_obeys_the_read_deadline() {
        let mut child = Command::new("sh")
            .args(["-c", "sleep 10"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("start silent child");
        let stdout = child.stdout.take().expect("silent child stdout");
        let started = Instant::now();
        let error = read_limited_line(&mut BufReader::new(stdout), Duration::from_millis(25))
            .expect_err("silent child must time out");
        assert!(error.contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(1));
        child.kill().expect("kill silent child");
        child.wait().expect("reap silent child");
    }
}
