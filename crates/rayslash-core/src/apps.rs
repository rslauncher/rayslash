mod app_discovery;
mod desktop_entry;
mod icon_lookup;

use std::path::PathBuf;

use crate::actions::CommandSpec;
use serde::{Deserialize, Serialize};

pub use app_discovery::{
    ApplicationScan, ApplicationScanStatistics, ApplicationSource, SourceScanStatistics,
    desktop_application_dirs, desktop_apps_cache_is_current, discover_and_cache_desktop_apps,
    discover_and_cache_desktop_apps_with_diagnostics, discover_desktop_apps,
    discover_desktop_apps_in_dirs, discover_desktop_apps_in_dirs_with_diagnostics,
    load_cached_desktop_apps, load_cached_desktop_scan,
};
pub use desktop_entry::{parse_desktop_entry, parse_exec_command};
pub use icon_lookup::resolve_desktop_icon_in_dirs;

/// The single final disposition assigned to a desktop-entry candidate.
///
/// These values are safe to aggregate in diagnostics: they contain no desktop-entry data,
/// paths, commands, or error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopCandidateOutcome {
    Indexed,
    Duplicate,
    Hidden,
    NoDisplay,
    UnsupportedType,
    MissingName,
    MissingExec,
    MalformedDesktopEntry,
    DesktopEnvironmentFiltered,
    InvalidTryExec,
    MissingExecutable,
    InvalidEncoding,
    ReadFailure,
    MetadataFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopAction {
    pub id: String,
    pub name: String,
    pub exec: Option<String>,
    pub command: Option<CommandSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub localized_names: Vec<String>,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub mime_types: Vec<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub actions: Vec<DesktopAction>,
    pub dbus_activatable: bool,
    pub startup_wm_class: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub command: CommandSpec,
    pub desktop_file: PathBuf,
}

impl DesktopApp {
    pub fn supports_mime_type(&self, mime_type: &str) -> bool {
        self.mime_types
            .iter()
            .any(|app_mime_type| app_mime_type == mime_type)
    }

    pub fn supports_directory_opening(&self) -> bool {
        self.supports_mime_type("inode/directory")
    }

    pub fn has_category(&self, category: &str) -> bool {
        self.categories
            .iter()
            .any(|app_category| app_category == category)
    }

    pub fn is_folder_opener_candidate(&self) -> bool {
        self.supports_directory_opening()
            || self.has_category("FileManager")
            || self.has_category("TerminalEmulator")
            || self.has_category("IDE")
    }

    pub fn is_terminal_emulator(&self) -> bool {
        self.has_category("TerminalEmulator")
    }
}
