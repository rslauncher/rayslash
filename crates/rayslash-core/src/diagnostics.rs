use crate::apps::ApplicationScanStatistics;

/// Minimal, vendor-neutral boundary for aggregate application-scan diagnostics.
///
/// The payload type deliberately cannot carry application names, paths, commands, queries, or
/// arbitrary errors. Discovery itself never performs network access.
pub trait Telemetry: Send + Sync {
    fn application_scan_completed(&self, statistics: &ApplicationScanStatistics);
}

#[derive(Debug, Default)]
pub struct NoopTelemetry;

impl Telemetry for NoopTelemetry {
    fn application_scan_completed(&self, _statistics: &ApplicationScanStatistics) {}
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
}
