use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use rayslash_core::{
    apps::ApplicationScanStatistics,
    diagnostics::{OperationalDiagnostic, OperationalDiagnosticStatistics, Telemetry},
};
use sentry::{Client, ClientOptions, Level, protocol::Event, types::Dsn};

const IDENTICAL_SCAN_SUPPRESSION: Duration = Duration::from_secs(10 * 60);
const IDENTICAL_OPERATIONAL_FAILURE_SUPPRESSION: Duration = Duration::from_secs(10 * 60);

pub(crate) struct DiagnosticsTelemetry {
    enabled: AtomicBool,
    latest: Mutex<Option<ApplicationScanStatistics>>,
    operational: Mutex<OperationalDiagnosticStatistics>,
    last_remote_scan: Mutex<Option<(u64, Instant)>>,
    last_remote_operational: Mutex<BTreeMap<OperationalDiagnostic, Instant>>,
    remote: Option<Arc<Client>>,
    environment: SafeEnvironment,
}

impl DiagnosticsTelemetry {
    pub(crate) fn new(enabled: bool) -> Arc<Self> {
        let environment = SafeEnvironment::detect();
        Arc::new(Self {
            enabled: AtomicBool::new(enabled),
            latest: Mutex::new(None),
            operational: Mutex::new(OperationalDiagnosticStatistics::default()),
            last_remote_scan: Mutex::new(None),
            last_remote_operational: Mutex::new(BTreeMap::new()),
            remote: sentry_client(),
            environment,
        })
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub(crate) fn record_cached_scan(&self, statistics: ApplicationScanStatistics) {
        *self
            .latest
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(statistics);
    }

    pub(crate) fn local_summary(&self) -> String {
        let scan = self
            .latest
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(ApplicationScanStatistics::local_summary)
            .unwrap_or_else(|| "No application scan has completed yet".to_owned());
        let failures = self
            .operational
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .failures;
        if failures == 0 {
            scan
        } else {
            format!("{scan} · {failures} operational failures")
        }
    }

    pub(crate) fn local_report(&self) -> String {
        let statistics = self
            .latest
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let operational = self
            .operational
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let (summary, read, parsed, consistent, structured) = statistics.as_ref().map_or_else(
            || {
                (
                    "No application scan has completed yet".to_owned(),
                    "unknown".to_owned(),
                    "unknown".to_owned(),
                    "unknown".to_owned(),
                    "null".to_owned(),
                )
            },
            |statistics| {
                (
                    statistics.local_summary(),
                    statistics.successfully_read().to_string(),
                    statistics.successfully_parsed().to_string(),
                    statistics.is_consistent().to_string(),
                    serde_json::to_string_pretty(statistics).unwrap_or_else(|_| "null".to_owned()),
                )
            },
        );
        let operational_structured =
            serde_json::to_string_pretty(&operational).unwrap_or_else(|_| "null".to_owned());
        format!(
            "Rayslash diagnostic report\nVersion: {}\nArchitecture: {}\nDistribution: {} {}\nDesktop: {}\nSession: {}\nInstallation: {}\n\nApplication discovery:\n{}\nSuccessfully read: {}\nSuccessfully parsed: {}\nAccounting consistent: {}\n\nStructured discovery aggregate:\n{}\n\nOperational failures:\n{} recorded\nAccounting consistent: {}\n\nStructured operational aggregate:\n{}\n\nPrivacy: this report contains aggregate counts, stable failure codes, and coarse error categories only; it excludes application names, paths, commands, searches, history, raw errors, and stack traces.",
            env!("CARGO_PKG_VERSION"),
            self.environment.architecture,
            self.environment.distribution,
            self.environment.distribution_major,
            self.environment.desktop,
            self.environment.session,
            self.environment.installation,
            summary,
            read,
            parsed,
            consistent,
            structured,
            operational.failures,
            operational.is_consistent(),
            operational_structured,
        )
    }

    fn send_scan(&self, statistics: &ApplicationScanStatistics) {
        if !self.enabled.load(Ordering::SeqCst) || cfg!(test) {
            return;
        }
        let Some(client) = self.remote.as_ref() else {
            return;
        };

        let mut hasher = DefaultHasher::new();
        serde_json::to_string(statistics)
            .unwrap_or_default()
            .hash(&mut hasher);
        let fingerprint = hasher.finish();
        let mut last = self
            .last_remote_scan
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if last.as_ref().is_some_and(|(previous, sent_at)| {
            *previous == fingerprint && sent_at.elapsed() < IDENTICAL_SCAN_SUPPRESSION
        }) {
            return;
        }
        *last = Some((fingerprint, Instant::now()));
        drop(last);

        let mut tags = self.environment.tags();
        tags.insert("event_schema".to_owned(), "application_scan_v1".to_owned());
        tags.insert(
            "accounting_consistent".to_owned(),
            statistics.is_consistent().to_string(),
        );
        let mut extra = BTreeMap::new();
        extra.insert(
            "scan".to_owned(),
            serde_json::to_value(statistics).unwrap_or(serde_json::Value::Null),
        );
        client.capture_event(
            Event {
                level: Level::Info,
                message: Some("application scan completed".to_owned()),
                logger: Some("rayslash.diagnostics".to_owned()),
                tags,
                extra,
                fingerprint: Cow::Owned(vec![Cow::Borrowed("application-scan-summary")]),
                ..Default::default()
            },
            None,
        );
    }

    fn send_operational_failure(&self, diagnostic: OperationalDiagnostic) {
        if !self.enabled.load(Ordering::SeqCst) || cfg!(test) {
            return;
        }
        let Some(client) = self.remote.as_ref() else {
            return;
        };

        let mut last = self
            .last_remote_operational
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let now = Instant::now();
        if last.get(&diagnostic).is_some_and(|sent_at| {
            now.duration_since(*sent_at) < IDENTICAL_OPERATIONAL_FAILURE_SUPPRESSION
        }) {
            return;
        }
        last.insert(diagnostic, now);
        drop(last);

        let code = serialized_enum_name(diagnostic.code);
        let mut tags = self.environment.tags();
        tags.insert("event_schema".to_owned(), "operational_v1".to_owned());
        tags.insert(
            "diagnostic_area".to_owned(),
            serialized_enum_name(diagnostic.code.area()),
        );
        tags.insert("diagnostic_code".to_owned(), code.clone());
        if let Some(kind) = diagnostic.io_error_kind {
            tags.insert("io_error_kind".to_owned(), serialized_enum_name(kind));
        }
        let mut extra = BTreeMap::new();
        extra.insert(
            "diagnostic".to_owned(),
            serde_json::to_value(diagnostic).unwrap_or(serde_json::Value::Null),
        );
        client.capture_event(
            Event {
                level: Level::Warning,
                message: Some(format!("operational diagnostic: {code}")),
                logger: Some("rayslash.diagnostics".to_owned()),
                tags,
                extra,
                fingerprint: Cow::Owned(vec![
                    Cow::Borrowed("rayslash-operational"),
                    Cow::Owned(code),
                ]),
                ..Default::default()
            },
            None,
        );
    }
}

impl Telemetry for DiagnosticsTelemetry {
    fn application_scan_completed(&self, statistics: &ApplicationScanStatistics) {
        eprintln!("application discovery: {}", statistics.local_summary());
        *self
            .latest
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(statistics.clone());
        self.send_scan(statistics);
        if !statistics.is_consistent() {
            self.operational_failure(OperationalDiagnostic::new(
                rayslash_core::diagnostics::OperationalDiagnosticCode::ApplicationScanInvariantViolation,
            ));
        }
    }

    fn operational_failure(&self, diagnostic: OperationalDiagnostic) {
        self.operational
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .record(diagnostic);
        self.send_operational_failure(diagnostic);
    }
}

fn serialized_enum_name(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn sentry_client() -> Option<Arc<Client>> {
    if cfg!(debug_assertions)
        && std::env::var_os("RAYSLASH_ENABLE_DEV_TELEMETRY").as_deref()
            != Some(std::ffi::OsStr::new("1"))
    {
        return None;
    }
    let dsn = option_env!("RAYSLASH_SENTRY_DSN")
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            std::env::var("RAYSLASH_SENTRY_DSN")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })?;
    let dsn = match dsn.parse::<Dsn>() {
        Ok(dsn) => dsn,
        Err(_) => {
            eprintln!("anonymous diagnostics are unavailable: RAYSLASH_SENTRY_DSN is invalid");
            return None;
        }
    };
    let mut options = ClientOptions::default();
    options.dsn = Some(dsn);
    options.release = Some(Cow::Owned(format!(
        "rayslash@{}",
        env!("CARGO_PKG_VERSION")
    )));
    options.environment = Some(Cow::Borrowed("production"));
    options.send_default_pii = false;
    options.server_name = None;
    options.default_integrations = false;
    options.auto_session_tracking = false;
    options.shutdown_timeout = Duration::from_millis(100);
    options.before_send = Some(Arc::new(scrub_event));
    let client = Client::from(sentry::apply_defaults(options));
    client.is_enabled().then(|| Arc::new(client))
}

fn scrub_event(mut event: Event<'static>) -> Option<Event<'static>> {
    // Defense in depth: only the explicitly constructed aggregate event fields survive.
    event.user = None;
    event.request = None;
    event.server_name = None;
    event.breadcrumbs = Default::default();
    event.contexts.clear();
    event.exception = Default::default();
    event.threads = Default::default();
    event.modules.clear();
    event.stacktrace = None;
    Some(event)
}

#[derive(Debug)]
struct SafeEnvironment {
    architecture: &'static str,
    distribution: String,
    distribution_major: String,
    desktop: String,
    session: &'static str,
    installation: &'static str,
}

impl SafeEnvironment {
    fn detect() -> Self {
        let (distribution, distribution_major) = distribution();
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .or_else(|| std::env::var("DESKTOP_SESSION").ok());
        Self {
            architecture: std::env::consts::ARCH,
            distribution,
            distribution_major,
            desktop: desktop_environment(desktop.as_deref()),
            session: session_type(std::env::var("XDG_SESSION_TYPE").ok().as_deref()),
            installation: installation_type(),
        }
    }

    fn tags(&self) -> BTreeMap<String, String> {
        [
            ("architecture", self.architecture.to_owned()),
            ("distribution", self.distribution.clone()),
            ("distribution_major", self.distribution_major.clone()),
            ("desktop", self.desktop.clone()),
            ("session", self.session.to_owned()),
            ("installation", self.installation.to_owned()),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
    }
}

fn distribution() -> (String, String) {
    let contents = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let id = os_release_value(&contents, "ID").unwrap_or_default();
    let distribution = match id.to_ascii_lowercase().as_str() {
        "arch" | "centos" | "debian" | "fedora" | "gentoo" | "linuxmint" | "manjaro" | "nixos"
        | "opensuse" | "opensuse-leap" | "pop" | "rhel" | "ubuntu" => id.to_ascii_lowercase(),
        "" => "unknown".to_owned(),
        _ => "other".to_owned(),
    };
    let major = os_release_value(&contents, "VERSION_ID")
        .and_then(|value| value.split('.').next())
        .filter(|value| !value.is_empty() && value.len() <= 4)
        .filter(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
        .unwrap_or("unknown")
        .to_owned();
    (distribution, major)
}

fn os_release_value<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    contents.lines().find_map(|line| {
        let (candidate, value) = line.split_once('=')?;
        (candidate == key).then(|| value.trim().trim_matches(['\'', '"']))
    })
}

fn desktop_environment(value: Option<&str>) -> String {
    let value = value.unwrap_or_default().to_ascii_lowercase();
    for desktop in [
        "budgie",
        "cinnamon",
        "cosmic",
        "deepin",
        "enlightenment",
        "gnome",
        "hyprland",
        "kde",
        "lxde",
        "lxqt",
        "mate",
        "pantheon",
        "sway",
        "unity",
        "xfce",
    ] {
        if value
            .split([':', ';', ','])
            .any(|item| item.trim() == desktop)
        {
            return desktop.to_owned();
        }
    }
    if value.is_empty() {
        "unknown".to_owned()
    } else {
        "other".to_owned()
    }
}

fn session_type(value: Option<&str>) -> &'static str {
    match value.unwrap_or_default().to_ascii_lowercase().as_str() {
        "wayland" => "wayland",
        "x11" => "x11",
        "tty" => "tty",
        _ => "unknown",
    }
}

fn installation_type() -> &'static str {
    if std::env::var_os("FLATPAK_ID").is_some() {
        "flatpak"
    } else if std::env::var_os("APPIMAGE").is_some() {
        "appimage"
    } else if std::env::var_os("SNAP").is_some() {
        "snap"
    } else {
        "system_or_source"
    }
}

#[cfg(test)]
mod tests {
    use rayslash_core::apps::{ApplicationSource, DesktopCandidateOutcome, SourceScanStatistics};
    use rayslash_core::diagnostics::{OperationalDiagnosticCode, SafeIoErrorKind};
    use sentry::protocol::{Request, User};

    use super::*;

    #[test]
    fn desktop_values_are_coarsened_to_a_fixed_allowlist() {
        assert_eq!(desktop_environment(Some("GNOME:GNOME-Classic")), "gnome");
        assert_eq!(desktop_environment(Some("Alice's custom desktop")), "other");
        assert_eq!(desktop_environment(None), "unknown");
    }

    #[test]
    fn os_release_parser_does_not_return_unrequested_fields() {
        let contents = "ID=fedora\nVERSION_ID=42\nPRETTY_NAME=Alice Linux\n";
        assert_eq!(os_release_value(contents, "ID"), Some("fedora"));
        assert_eq!(os_release_value(contents, "VERSION_ID"), Some("42"));
    }

    #[test]
    fn disabled_telemetry_keeps_local_diagnostics_without_a_remote_client() {
        let telemetry = DiagnosticsTelemetry {
            enabled: AtomicBool::new(false),
            latest: Mutex::new(None),
            operational: Mutex::new(OperationalDiagnosticStatistics::default()),
            last_remote_scan: Mutex::new(None),
            last_remote_operational: Mutex::new(BTreeMap::new()),
            remote: None,
            environment: SafeEnvironment::detect(),
        };
        telemetry.application_scan_completed(&ApplicationScanStatistics::default());
        assert!(telemetry.local_summary().contains("0 candidates"));
        assert!(
            telemetry
                .last_remote_scan
                .lock()
                .expect("remote scan lock")
                .is_none()
        );
    }

    #[test]
    fn final_event_scrub_removes_identity_and_request_fields() {
        let event = Event {
            user: Some(User {
                username: Some("private-user".to_owned()),
                ..Default::default()
            }),
            request: Some(Request {
                url: Some("https://example.invalid/private-path".parse().expect("URL")),
                ..Default::default()
            }),
            server_name: Some(Cow::Borrowed("private-hostname")),
            ..Default::default()
        };

        let scrubbed = scrub_event(event).expect("aggregate event is retained");

        assert!(scrubbed.user.is_none());
        assert!(scrubbed.request.is_none());
        assert!(scrubbed.server_name.is_none());
    }

    #[test]
    fn local_report_includes_complete_sanitized_source_accounting() {
        let telemetry = DiagnosticsTelemetry {
            enabled: AtomicBool::new(false),
            latest: Mutex::new(None),
            operational: Mutex::new(OperationalDiagnosticStatistics::default()),
            last_remote_scan: Mutex::new(None),
            last_remote_operational: Mutex::new(BTreeMap::new()),
            remote: None,
            environment: SafeEnvironment::detect(),
        };
        telemetry.record_cached_scan(ApplicationScanStatistics {
            candidates: 1,
            source_errors: 0,
            outcomes: [(DesktopCandidateOutcome::MissingExecutable, 1)].into(),
            sources: [(
                ApplicationSource::UserFlatpak,
                SourceScanStatistics {
                    candidates: 1,
                    source_errors: 0,
                    outcomes: [(DesktopCandidateOutcome::MissingExecutable, 1)].into(),
                },
            )]
            .into(),
        });

        let report = telemetry.local_report();

        assert!(report.contains("Successfully read: 1"));
        assert!(report.contains("Successfully parsed: 1"));
        assert!(report.contains("Accounting consistent: true"));
        assert!(report.contains("\"user_flatpak\""));
        assert!(report.contains("\"missing_executable\": 1"));
    }

    #[test]
    fn operational_failures_are_aggregate_and_exclude_raw_error_details() {
        let telemetry = DiagnosticsTelemetry {
            enabled: AtomicBool::new(false),
            latest: Mutex::new(None),
            operational: Mutex::new(OperationalDiagnosticStatistics::default()),
            last_remote_scan: Mutex::new(None),
            last_remote_operational: Mutex::new(BTreeMap::new()),
            remote: None,
            environment: SafeEnvironment::detect(),
        };
        telemetry.operational_failure(OperationalDiagnostic {
            code: OperationalDiagnosticCode::MainConfigurationSave,
            io_error_kind: Some(SafeIoErrorKind::PermissionDenied),
        });

        let report = telemetry.local_report();

        assert!(report.contains("1 recorded"));
        assert!(report.contains("\"main_configuration_save\""));
        assert!(report.contains("\"permission_denied\""));
        assert!(!report.contains("/home/"));
        assert!(telemetry.local_summary().contains("1 operational failures"));
    }

    #[test]
    fn enum_tags_use_stable_snake_case_values() {
        assert_eq!(
            serialized_enum_name(OperationalDiagnosticCode::ModuleRuntimeInvalidResponse),
            "module_runtime_invalid_response"
        );
        assert_eq!(
            serialized_enum_name(OperationalDiagnosticCode::ModuleRuntimeInvalidResponse.area()),
            "modules"
        );
    }
}
