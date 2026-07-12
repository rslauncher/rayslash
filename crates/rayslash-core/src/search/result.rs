use std::path::{Path, PathBuf};

use crate::{
    actions::CommandSpec,
    providers::{ProviderAction, ProviderId},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub flair: String,
    pub subtitle: String,
    pub icon: SearchResultIcon,
    pub kind: SearchResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultIcon {
    Placeholder,
    App {
        path: Option<PathBuf>,
    },
    ProjectFolder,
    Module {
        label: String,
        path: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleAction {
    CopyText(String),
    OpenUrl(String),
    OpenPath(PathBuf),
    ShowMessage(String),
    Notify {
        title: String,
        body: String,
    },
    RunApprovedCommand(Vec<String>),
    ScheduleNotification {
        delay: u64,
        title: String,
        body: String,
    },
    ScheduleCommand {
        delay: u64,
        command: Vec<String>,
    },
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultKind {
    Placeholder,
    NoResults {
        query: String,
    },
    App {
        id: String,
        command: CommandSpec,
        desktop_file: PathBuf,
        dbus_activatable: bool,
        startup_wm_class: Option<String>,
    },
    Project {
        path: PathBuf,
    },
    Module {
        module_id: String,
        result_id: String,
        action: ModuleAction,
        score: Option<u32>,
    },
}

impl SearchResult {
    pub fn provider_id(&self) -> ProviderId {
        match &self.kind {
            SearchResultKind::App { .. } => ProviderId::CORE_APPS,
            SearchResultKind::Project { .. } => ProviderId::CORE_FOLDERS,
            SearchResultKind::Module { module_id, .. } => ProviderId::new(module_id.clone()),
            SearchResultKind::Placeholder | SearchResultKind::NoResults { .. } => {
                ProviderId::CORE_FALLBACK
            }
        }
    }

    pub fn provider_action(&self) -> ProviderAction {
        ProviderAction::from_result(self)
    }

    pub fn project_path(&self) -> Option<&Path> {
        match &self.kind {
            SearchResultKind::Project { path } => Some(path),
            _ => None,
        }
    }

    pub fn app_command(&self) -> Option<&CommandSpec> {
        match &self.kind {
            SearchResultKind::App { command, .. } => Some(command),
            _ => None,
        }
    }

    pub fn is_no_results(&self) -> bool {
        matches!(self.kind, SearchResultKind::NoResults { .. })
    }

    pub fn app_id(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::App { id, .. } => Some(id),
            _ => None,
        }
    }

    pub fn app_activation(&self) -> Option<AppActivation<'_>> {
        match &self.kind {
            SearchResultKind::App {
                id,
                command,
                desktop_file,
                dbus_activatable,
                startup_wm_class,
            } => Some(AppActivation {
                id,
                command,
                desktop_file,
                dbus_activatable: *dbus_activatable,
                startup_wm_class: startup_wm_class.as_deref(),
            }),
            _ => None,
        }
    }

    pub fn stable_id(&self) -> Option<String> {
        match &self.kind {
            SearchResultKind::App { id, .. } => Some(format!("app:{id}")),
            SearchResultKind::Project { path } => Some(format!("folder:{}", path.display())),
            SearchResultKind::Module {
                module_id,
                result_id,
                ..
            } => Some(format!("module:{module_id}:{result_id}")),
            SearchResultKind::NoResults { query } => Some(format!("no-results:{}", query.trim())),
            SearchResultKind::Placeholder => None,
        }
    }

    pub fn learning_id(&self) -> Option<String> {
        match self.kind {
            SearchResultKind::App { .. } | SearchResultKind::Project { .. } => self.stable_id(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AppActivation<'a> {
    pub id: &'a str,
    pub command: &'a CommandSpec,
    pub desktop_file: &'a Path,
    pub dbus_activatable: bool,
    pub startup_wm_class: Option<&'a str>,
}
