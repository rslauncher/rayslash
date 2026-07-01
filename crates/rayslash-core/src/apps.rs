mod app_discovery;
mod desktop_entry;
mod icon_lookup;

use std::path::PathBuf;

use crate::actions::CommandSpec;

pub use app_discovery::{discover_desktop_apps, discover_desktop_apps_in_dirs};
pub use desktop_entry::{parse_desktop_entry, parse_exec_command};
pub use icon_lookup::resolve_desktop_icon_in_dirs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub mime_types: Vec<String>,
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
}
