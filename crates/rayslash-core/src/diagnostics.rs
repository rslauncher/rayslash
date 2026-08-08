use std::{collections::BTreeMap, io};

use serde::{Deserialize, Serialize};

use crate::apps::ApplicationScanStatistics;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationalDiagnosticArea {
    Discovery,
    Configuration,
    Persistence,
    Launcher,
    Ipc,
    Action,
    Modules,
    Updates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationalDiagnosticCode {
    ApplicationScanInvariantViolation,
    MainConfigurationLoad,
    MainConfigurationSave,
    ModuleConfigurationLoad,
    ModuleConfigurationSave,
    ApplicationStateLoad,
    ApplicationStateSave,
    RankingStateLoad,
    RankingStateSave,
    RankingStateClear,
    WindowShow,
    WindowHide,
    WindowUiDispatch,
    DesktopWatcherInitialize,
    DesktopWatcherWatch,
    ProjectWatcherInitialize,
    ProjectWatcherWatch,
    IpcAccept,
    IpcRead,
    IpcUiDispatch,
    ApplicationLaunch,
    FolderLaunch,
    ModuleActionLaunch,
    ScheduledActionLaunch,
    ClipboardWrite,
    DiagnosticReportCopy,
    ModuleRegistryRefresh,
    ModuleInstalledStateLoad,
    ModuleInstall,
    ModuleUpdate,
    ModuleRemove,
    ModuleRuntimeHostUnavailable,
    ModuleRuntimeCompilerUnavailable,
    ModuleRuntimeTimeout,
    ModuleRuntimeProtocol,
    ModuleRuntimeInvalidResponse,
    ModuleRuntimeOther,
    AppUpdateCheck,
    AppUpdateDownload,
    AppUpdateInstall,
}

impl OperationalDiagnosticCode {
    pub const fn area(self) -> OperationalDiagnosticArea {
        match self {
            Self::ApplicationScanInvariantViolation => OperationalDiagnosticArea::Discovery,
            Self::MainConfigurationLoad
            | Self::MainConfigurationSave
            | Self::ModuleConfigurationLoad
            | Self::ModuleConfigurationSave => OperationalDiagnosticArea::Configuration,
            Self::ApplicationStateLoad
            | Self::ApplicationStateSave
            | Self::RankingStateLoad
            | Self::RankingStateSave
            | Self::RankingStateClear => OperationalDiagnosticArea::Persistence,
            Self::WindowShow
            | Self::WindowHide
            | Self::WindowUiDispatch
            | Self::DesktopWatcherInitialize
            | Self::DesktopWatcherWatch
            | Self::ProjectWatcherInitialize
            | Self::ProjectWatcherWatch => OperationalDiagnosticArea::Launcher,
            Self::IpcAccept | Self::IpcRead | Self::IpcUiDispatch => OperationalDiagnosticArea::Ipc,
            Self::ApplicationLaunch
            | Self::FolderLaunch
            | Self::ModuleActionLaunch
            | Self::ScheduledActionLaunch
            | Self::ClipboardWrite
            | Self::DiagnosticReportCopy => OperationalDiagnosticArea::Action,
            Self::ModuleRegistryRefresh
            | Self::ModuleInstalledStateLoad
            | Self::ModuleInstall
            | Self::ModuleUpdate
            | Self::ModuleRemove
            | Self::ModuleRuntimeHostUnavailable
            | Self::ModuleRuntimeCompilerUnavailable
            | Self::ModuleRuntimeTimeout
            | Self::ModuleRuntimeProtocol
            | Self::ModuleRuntimeInvalidResponse
            | Self::ModuleRuntimeOther => OperationalDiagnosticArea::Modules,
            Self::AppUpdateCheck | Self::AppUpdateDownload | Self::AppUpdateInstall => {
                OperationalDiagnosticArea::Updates
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafeIoErrorKind {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidData,
    TimedOut,
    WouldBlock,
    Connection,
    Address,
    Interrupted,
    Unsupported,
    OutOfMemory,
    Other,
}

impl From<io::ErrorKind> for SafeIoErrorKind {
    fn from(kind: io::ErrorKind) -> Self {
        match kind {
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            io::ErrorKind::AlreadyExists => Self::AlreadyExists,
            io::ErrorKind::InvalidInput
            | io::ErrorKind::InvalidData
            | io::ErrorKind::UnexpectedEof => Self::InvalidData,
            io::ErrorKind::TimedOut => Self::TimedOut,
            io::ErrorKind::WouldBlock => Self::WouldBlock,
            io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::NotConnected
            | io::ErrorKind::BrokenPipe => Self::Connection,
            io::ErrorKind::AddrInUse | io::ErrorKind::AddrNotAvailable => Self::Address,
            io::ErrorKind::Interrupted => Self::Interrupted,
            io::ErrorKind::Unsupported => Self::Unsupported,
            io::ErrorKind::OutOfMemory => Self::OutOfMemory,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OperationalDiagnostic {
    pub code: OperationalDiagnosticCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub io_error_kind: Option<SafeIoErrorKind>,
}

impl OperationalDiagnostic {
    pub const fn new(code: OperationalDiagnosticCode) -> Self {
        Self {
            code,
            io_error_kind: None,
        }
    }

    pub fn from_io(code: OperationalDiagnosticCode, error: &io::Error) -> Self {
        Self {
            code,
            io_error_kind: Some(error.kind().into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OperationalDiagnosticCount {
    pub occurrences: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub io_error_kinds: BTreeMap<SafeIoErrorKind, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OperationalDiagnosticStatistics {
    pub failures: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub codes: BTreeMap<OperationalDiagnosticCode, OperationalDiagnosticCount>,
}

impl OperationalDiagnosticStatistics {
    pub fn record(&mut self, diagnostic: OperationalDiagnostic) {
        self.failures = self.failures.saturating_add(1);
        let count = self.codes.entry(diagnostic.code).or_default();
        count.occurrences = count.occurrences.saturating_add(1);
        if let Some(kind) = diagnostic.io_error_kind {
            let occurrences = count.io_error_kinds.entry(kind).or_default();
            *occurrences = occurrences.saturating_add(1);
        }
    }

    pub fn is_consistent(&self) -> bool {
        self.failures
            == self
                .codes
                .values()
                .map(|count| count.occurrences)
                .sum::<u64>()
            && self
                .codes
                .values()
                .all(|count| count.io_error_kinds.values().sum::<u64>() <= count.occurrences)
    }
}

/// Minimal, vendor-neutral boundary for aggregate discovery and operational diagnostics.
///
/// The payload type deliberately cannot carry application names, paths, commands, queries, or
/// arbitrary errors. Instrumented operations never perform network access through this boundary;
/// the UI adapter decides whether an explicitly opted-in remote sink is available.
pub trait Telemetry: Send + Sync {
    fn application_scan_completed(&self, statistics: &ApplicationScanStatistics);

    fn operational_failure(&self, diagnostic: OperationalDiagnostic);
}

#[derive(Debug, Default)]
pub struct NoopTelemetry;

impl Telemetry for NoopTelemetry {
    fn application_scan_completed(&self, _statistics: &ApplicationScanStatistics) {}

    fn operational_failure(&self, _diagnostic: OperationalDiagnostic) {}
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::apps::{ApplicationSource, DesktopCandidateOutcome, SourceScanStatistics};

    use super::*;

    #[derive(Default)]
    struct RecordingTelemetry {
        scans: Mutex<Vec<ApplicationScanStatistics>>,
    }

    impl Telemetry for RecordingTelemetry {
        fn application_scan_completed(&self, statistics: &ApplicationScanStatistics) {
            self.scans
                .lock()
                .expect("recording telemetry lock")
                .push(statistics.clone());
        }

        fn operational_failure(&self, _diagnostic: OperationalDiagnostic) {}
    }

    #[test]
    fn noop_telemetry_is_safe_when_diagnostics_are_disabled() {
        NoopTelemetry.application_scan_completed(&ApplicationScanStatistics::default());
    }

    #[test]
    fn telemetry_boundary_only_receives_sanitized_aggregate_types() {
        let telemetry = RecordingTelemetry::default();
        let statistics = ApplicationScanStatistics {
            candidates: 1,
            source_errors: 0,
            outcomes: [(DesktopCandidateOutcome::ReadFailure, 1)].into(),
            sources: [(
                ApplicationSource::SystemXdg,
                SourceScanStatistics {
                    candidates: 1,
                    source_errors: 0,
                    outcomes: [(DesktopCandidateOutcome::ReadFailure, 1)].into(),
                },
            )]
            .into(),
        };

        telemetry.application_scan_completed(&statistics);

        assert_eq!(
            telemetry
                .scans
                .lock()
                .expect("recording telemetry lock")
                .as_slice(),
            &[statistics]
        );
    }

    #[test]
    fn operational_statistics_are_typed_and_consistent() {
        let mut statistics = OperationalDiagnosticStatistics::default();
        statistics.record(OperationalDiagnostic::from_io(
            OperationalDiagnosticCode::ApplicationStateSave,
            &io::Error::new(
                io::ErrorKind::PermissionDenied,
                "/home/alice/.local/private-state",
            ),
        ));
        statistics.record(OperationalDiagnostic::new(
            OperationalDiagnosticCode::ModuleRuntimeProtocol,
        ));

        assert!(statistics.is_consistent());
        assert_eq!(statistics.failures, 2);
        let serialized = serde_json::to_string(&statistics).expect("serialize diagnostics");
        assert!(serialized.contains("application_state_save"));
        assert!(serialized.contains("permission_denied"));
        assert!(!serialized.contains("alice"));
        assert!(!serialized.contains("private-state"));
    }
}
