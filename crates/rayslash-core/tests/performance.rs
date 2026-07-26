use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs,
    hint::black_box,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};

use rayslash_core::{
    actions::{self, CommandSpec},
    apps::{self, DesktopApp},
    config::{self, ProviderConfig},
    modules::{InstalledModule, InstalledModules, ModulePackageManifest, ModulesConfig},
    projects::{self, Project},
    ranking::RankingState,
    search,
};
use serde_json::json;

mod fixtures;
use fixtures::TempDir;

#[test]
#[ignore = "diagnostic probe; run with --ignored --nocapture when investigating search latency"]
fn mixed_search_performance_probe() {
    let app_count = repetitions("RAYSLASH_SEARCH_APP_COUNT", 4_000);
    let project_count = repetitions("RAYSLASH_SEARCH_PROJECT_COUNT", 1_000);
    let sample_count = repetitions("RAYSLASH_SEARCH_SAMPLES", 200);
    let warmups = repetitions("RAYSLASH_SEARCH_WARMUPS", 20);
    let result_limit = std::env::var("RAYSLASH_SEARCH_RESULT_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    let apps = (0..app_count).map(synthetic_app).collect::<Vec<_>>();
    let projects = (0..project_count)
        .map(synthetic_project)
        .collect::<Vec<_>>();
    let providers = ProviderConfig::default();
    let ranking = RankingState::default();
    let queries = ["", "app 39", "editor", "project 42", "999 * 42"];

    for query in queries {
        for _ in 0..warmups {
            black_box(measured_search(
                &projects,
                &apps,
                query,
                &providers,
                &ranking,
                result_limit,
            ));
        }
        let mut samples = Vec::with_capacity(sample_count);
        let mut total_results = 0usize;
        for _ in 0..sample_count {
            let started = Instant::now();
            let results =
                measured_search(&projects, &apps, query, &providers, &ranking, result_limit);
            samples.push(started.elapsed());
            total_results += results.len();
            black_box(results);
        }
        print_distribution(
            &format!(
                "synthetic-search query={query:?} items={} limit={result_limit:?} results/sample={}",
                apps.len() + projects.len(),
                total_results / samples.len()
            ),
            &samples,
        );
    }
}

fn measured_search(
    projects: &[Project],
    apps: &[DesktopApp],
    query: &str,
    providers: &ProviderConfig,
    ranking: &RankingState,
    result_limit: Option<usize>,
) -> Vec<search::SearchResult> {
    result_limit.map_or_else(
        || search::mixed_results_with_ranking(projects, apps, &[], query, providers, Some(ranking)),
        |limit| {
            search::mixed_results_with_ranking_and_web_searches_limited(
                projects,
                apps,
                &[],
                &[],
                query,
                providers,
                Some(ranking),
                limit,
            )
        },
    )
}

#[test]
#[ignore = "diagnostic probe over the current desktop and configured folder sources"]
fn live_catalog_performance_probe() {
    let config = config::load_config().unwrap_or_default();

    let mut app_samples = Vec::with_capacity(20);
    let mut app_count = 0;
    for _ in 0..20 {
        let started = Instant::now();
        let discovered = apps::discover_desktop_apps();
        app_samples.push(started.elapsed());
        app_count = discovered.len();
        black_box(discovered);
    }
    print_distribution(&format!("desktop-discovery apps={app_count}"), &app_samples);

    let _ = apps::discover_and_cache_desktop_apps();
    let mut freshness_samples = Vec::with_capacity(100);
    for _ in 0..100 {
        let started = Instant::now();
        assert!(apps::desktop_apps_cache_is_current());
        freshness_samples.push(started.elapsed());
    }
    print_distribution(
        "desktop-cache source-metadata validation",
        &freshness_samples,
    );

    let mut folder_samples = Vec::with_capacity(100);
    let mut folder_count = 0;
    for _ in 0..100 {
        let started = Instant::now();
        let discovered = projects::scan_project_roots(&config.folder_sources);
        folder_samples.push(started.elapsed());
        folder_count = discovered.len();
        black_box(discovered);
    }
    print_distribution(
        &format!(
            "folder-discovery roots={} folders={folder_count}",
            config.folder_sources.len()
        ),
        &folder_samples,
    );
}

#[test]
#[ignore = "diagnostic probe for local process-spawn action dispatch"]
fn action_dispatch_performance_probe() {
    let command = CommandSpec {
        program: OsString::from("true"),
        args: Vec::new(),
    };
    let mut samples = Vec::with_capacity(100);
    for _ in 0..100 {
        let started = Instant::now();
        let mut child = actions::launch_app(&command).expect("spawn `true`");
        samples.push(started.elapsed());
        child.wait().expect("wait for `true`");
    }
    print_distribution("action-dispatch spawn acknowledgement (`true`)", &samples);
}

#[test]
#[ignore = "diagnostic probe for app activation including an unsuccessful wmctrl focus attempt"]
fn app_activation_performance_probe() {
    let command = CommandSpec {
        program: OsString::from("true"),
        args: Vec::new(),
    };
    let mut samples = Vec::with_capacity(20);
    for _ in 0..20 {
        let started = Instant::now();
        let outcome = actions::activate_app(
            "dev.rayslash.performance.DoesNotExist.desktop",
            "rayslash performance nonexistent app",
            &command,
            Path::new("/tmp/dev.rayslash.performance.DoesNotExist.desktop"),
            false,
            Some("rayslash-performance-nonexistent"),
        )
        .expect("activate synthetic app");
        samples.push(started.elapsed());
        if let actions::LaunchOutcome::Spawned(mut child) = outcome {
            child.wait().expect("wait for synthetic app");
        }
    }
    print_distribution(
        "app-activation unsuccessful focus probes + spawn acknowledgement (`true`)",
        &samples,
    );
}

#[test]
#[ignore = "requires RAYSLASH_MODULE_HOST, RAYSLASH_MODULE_WASM, and optionally query/settings/origins"]
fn module_host_performance_probe() {
    let host = required_path("RAYSLASH_MODULE_HOST");
    let module = required_path("RAYSLASH_MODULE_WASM");
    let temp = TempDir::new("rayslash-module-host-performance");
    let cache_dir = temp.join("cache");
    let query = std::env::var("RAYSLASH_MODULE_QUERY").unwrap_or_else(|_| "noop".into());
    let settings = std::env::var("RAYSLASH_MODULE_SETTINGS_JSON").unwrap_or_else(|_| "{}".into());
    let origins = std::env::var("RAYSLASH_MODULE_NETWORK_ORIGINS")
        .map(|value| {
            value
                .split(',')
                .filter(|origin| !origin.trim().is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let cold_repetitions = repetitions("RAYSLASH_MODULE_COLD_SAMPLES", 20);
    let warm_repetitions = repetitions("RAYSLASH_MODULE_WARM_SAMPLES", 200);
    let warmups = repetitions("RAYSLASH_MODULE_WARMUPS", 20);

    let mut cold_samples = Vec::with_capacity(cold_repetitions);
    for _ in 0..cold_repetitions {
        let started = Instant::now();
        let mut process = ModuleHost::start(&host, &module, &origins, &cache_dir);
        process.query(&query, &settings);
        cold_samples.push(started.elapsed());
    }
    print_distribution(
        &format!(
            "module-host cold-start+handshake+first-query module={} query={query:?}",
            module.display()
        ),
        &cold_samples,
    );

    let mut process = ModuleHost::start(&host, &module, &origins, &cache_dir);
    for _ in 0..warmups {
        process.query(&query, &settings);
    }
    let mut warm_samples = Vec::with_capacity(warm_repetitions);
    for _ in 0..warm_repetitions {
        let started = Instant::now();
        process.query(&query, &settings);
        warm_samples.push(started.elapsed());
    }
    print_distribution(
        &format!(
            "module-host warm-query module={} query={query:?}",
            module.display()
        ),
        &warm_samples,
    );
    let (rss_kib, peak_kib) = process.memory_kib();
    println!(
        "module-host resident-memory module={} rss={rss_kib}KiB peak={peak_kib}KiB",
        module.display()
    );
}

#[test]
#[ignore = "requires RAYSLASH_MODULE_HOST and RAYSLASH_MODULES_ROOT"]
fn installed_module_fanout_performance_probe() {
    let host = required_path("RAYSLASH_MODULE_HOST");
    let modules_root = required_existing_path("RAYSLASH_MODULES_ROOT");
    let query = std::env::var("RAYSLASH_MODULE_QUERY").unwrap_or_else(|_| "noop".into());
    let temp = TempDir::new("rayslash-module-fanout-performance");
    let data_home = temp.create_dir_all("data").expect("create data home");
    let state_home = temp.create_dir_all("state").expect("create state home");
    let cache_home = temp.create_dir_all("cache").expect("create cache home");
    let _environment = EnvironmentGuard::set(&[
        ("XDG_DATA_HOME", data_home.as_os_str()),
        ("XDG_STATE_HOME", state_home.as_os_str()),
        ("XDG_CACHE_HOME", cache_home.as_os_str()),
        ("RAYSLASH_MODULE_HOST", host.as_os_str()),
    ]);

    let module_dirs = [
        "rayslash-module-calculator",
        "rayslash-module-units",
        "rayslash-module-time",
        "rayslash-module-timers",
        "rayslash-module-currency",
        "rayslash-module-web-search",
        "rayslash-module-aliases",
    ];
    let mut installed = InstalledModules::default();
    let mut config = ModulesConfig::empty();
    for (index, directory) in module_dirs.into_iter().enumerate() {
        let source = modules_root.join(directory);
        let manifest_text = fs::read_to_string(source.join("module.toml"))
            .unwrap_or_else(|error| panic!("read {directory} manifest: {error}"));
        let manifest: ModulePackageManifest = toml::from_str(&manifest_text)
            .unwrap_or_else(|error| panic!("parse {directory} manifest: {error}"));
        let digest = format!("{:x}", index + 1).repeat(64);
        let digest = &digest[..64];
        let install_path = data_home
            .join("rayslash/modules")
            .join(&manifest.id)
            .join(format!("{}-{}", manifest.version, &digest[..16]));
        fs::create_dir_all(&install_path).expect("create module install directory");
        fs::write(install_path.join("module.toml"), &manifest_text).expect("copy module manifest");
        let crate_name = directory.replace('-', "_");
        fs::copy(
            source
                .join("target/wasm32-unknown-unknown/release")
                .join(format!("{crate_name}.wasm")),
            install_path.join("module.wasm"),
        )
        .expect("copy module component");
        installed.modules.insert(
            manifest.id.clone(),
            InstalledModule {
                version: manifest.version.clone(),
                digest: digest.to_owned(),
                source: manifest.source.clone(),
                source_commit: "a".repeat(40),
                install_path,
                enabled: true,
                permissions: manifest.permissions.clone(),
            },
        );
        config.set_installed(&manifest.id, &manifest.version.to_string(), true);
    }
    temp.write(
        "state/rayslash/modules/installed.toml",
        toml::to_string_pretty(&installed).expect("serialize installed state"),
    )
    .expect("write installed state");

    let mut settings = BTreeMap::new();
    if let (Ok(module_id), Ok(settings_json)) = (
        std::env::var("RAYSLASH_MODULE_SETTINGS_ID"),
        std::env::var("RAYSLASH_MODULE_SETTINGS_JSON"),
    ) {
        settings.insert(module_id, settings_json);
    }
    let started = Instant::now();
    let cold = rayslash_core::modules::query_installed_modules(&query, 20, &config, &settings);
    let cold_elapsed = started.elapsed();
    assert!(cold.errors.is_empty(), "fan-out errors: {:?}", cold.errors);
    println!(
        "installed-module-dispatch cold query={query:?} modules={} results={} elapsed={cold_elapsed:.3?}",
        installed.modules.len(),
        cold.results.len(),
    );

    for _ in 0..20 {
        black_box(rayslash_core::modules::query_installed_modules(
            &query, 20, &config, &settings,
        ));
    }
    let mut warm_samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = Instant::now();
        let results =
            rayslash_core::modules::query_installed_modules(&query, 20, &config, &settings);
        warm_samples.push(started.elapsed());
        assert!(
            results.errors.is_empty(),
            "fan-out errors: {:?}",
            results.errors
        );
        black_box(results);
    }
    print_distribution(
        &format!(
            "installed-module-dispatch warm query={query:?} modules={}",
            installed.modules.len()
        ),
        &warm_samples,
    );
}

struct ModuleHost {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl ModuleHost {
    fn start(host: &Path, module: &Path, origins: &[String], cache_dir: &Path) -> Self {
        let mut command = Command::new(host);
        command
            .arg("--module")
            .arg(module)
            .arg("--cache-dir")
            .arg(cache_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for origin in origins {
            command.arg("--network-origin").arg(origin);
        }
        let mut child = command.spawn().expect("start module host");
        let stdin = child.stdin.take().expect("module host stdin");
        let stdout = child.stdout.take().expect("module host stdout");
        let mut process = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };
        process.write(&json!({"type": "handshake", "protocol": 1}));
        let response = process.read();
        assert_eq!(response["type"], "handshake", "bad handshake: {response}");
        process
    }

    fn query(&mut self, query: &str, settings_json: &str) {
        let id = self.next_id;
        self.next_id += 1;
        self.write(&json!({
            "type": "query",
            "id": id,
            "query": query,
            "max_results": 20,
            "locale": null,
            "settings_json": settings_json,
        }));
        let response = self.read();
        assert_eq!(response["type"], "query", "bad query response: {response}");
        assert_eq!(response["id"], id, "wrong query ID: {response}");
        assert!(response.get("error").is_none(), "module error: {response}");
        black_box(response);
    }

    fn write(&mut self, value: &serde_json::Value) {
        serde_json::to_writer(&mut self.stdin, value).expect("serialize host request");
        self.stdin.write_all(b"\n").expect("write host request");
        self.stdin.flush().expect("flush host request");
    }

    fn read(&mut self) -> serde_json::Value {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read host response");
        assert!(!line.is_empty(), "module host closed stdout");
        serde_json::from_str(&line).expect("parse host response")
    }

    fn memory_kib(&self) -> (u64, u64) {
        let status = fs::read_to_string(format!("/proc/{}/status", self.child.id()))
            .expect("read module host process status");
        (status_kib(&status, "VmRSS:"), status_kib(&status, "VmHWM:"))
    }
}

impl Drop for ModuleHost {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn required_path(name: &str) -> PathBuf {
    let path = required_existing_path(name);
    assert!(path.is_file(), "{name} is not a file: {}", path.display());
    path
}

fn required_existing_path(name: &str) -> PathBuf {
    let path = std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} must point to an executable or WebAssembly component"));
    assert!(path.exists(), "{name} does not exist: {}", path.display());
    path
}

struct EnvironmentGuard(Vec<(String, Option<OsString>)>);

impl EnvironmentGuard {
    fn set(values: &[(&str, &OsStr)]) -> Self {
        let previous = values
            .iter()
            .map(|(name, _)| ((*name).to_owned(), std::env::var_os(name)))
            .collect();
        for (name, value) in values {
            // This ignored probe is run by exact name, so no other test thread can observe it.
            unsafe { std::env::set_var(name, value) };
        }
        Self(previous)
    }
}

impl Drop for EnvironmentGuard {
    fn drop(&mut self) {
        for (name, value) in self.0.drain(..) {
            // Restore the process environment before temporary paths are removed.
            unsafe {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                } else {
                    std::env::remove_var(name);
                }
            }
        }
    }
}

fn repetitions(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn status_kib(status: &str, field: &str) -> u64 {
    status
        .lines()
        .find_map(|line| {
            line.strip_prefix(field)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
        .unwrap_or_default()
}

fn print_distribution(label: &str, samples: &[Duration]) {
    assert!(!samples.is_empty());
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let total = sorted.iter().map(Duration::as_nanos).sum::<u128>();
    let mean = Duration::from_nanos((total / sorted.len() as u128) as u64);
    println!(
        "{label}: samples={} min={:.3?} p50={:.3?} p95={:.3?} max={:.3?} mean={:.3?}",
        sorted.len(),
        sorted[0],
        percentile(&sorted, 50),
        percentile(&sorted, 95),
        sorted[sorted.len() - 1],
        mean,
    );
}

fn percentile(samples: &[Duration], percentile: usize) -> Duration {
    let index = (samples.len() - 1) * percentile / 100;
    samples[index]
}

fn synthetic_app(index: usize) -> DesktopApp {
    DesktopApp {
        id: format!("dev.rayslash.fixture.App{index}.desktop"),
        name: format!("Fixture App {index}"),
        localized_names: vec![format!("Localized Fixture App {index}")],
        generic_name: Some(if index.is_multiple_of(3) {
            "Text Editor".to_owned()
        } else {
            "Application".to_owned()
        }),
        comment: Some(format!(
            "Synthetic app used for search performance probe {index}"
        )),
        exec: "true".to_owned(),
        icon: None,
        mime_types: Vec::new(),
        categories: vec!["Utility".to_owned()],
        keywords: vec![
            "fixture".to_owned(),
            "performance".to_owned(),
            format!("group{}", index % 100),
        ],
        actions: Vec::new(),
        dbus_activatable: false,
        startup_wm_class: None,
        icon_path: None,
        command: CommandSpec {
            program: OsString::from("true"),
            args: Vec::new(),
        },
        desktop_file: PathBuf::from(format!("/tmp/rayslash-fixture-{index}.desktop")),
    }
}

fn synthetic_project(index: usize) -> Project {
    Project {
        name: format!("Fixture Project {index}"),
        path: PathBuf::from(format!("/tmp/rayslash-fixture-project-{index}")),
    }
}
