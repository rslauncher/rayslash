use std::path::PathBuf;

use crate::config::{AliasConfig, AliasKind};

pub fn alias_kind(alias: &AliasConfig) -> AliasKind {
    alias
        .kind
        .unwrap_or_else(|| infer_alias_kind(&alias.target))
}

pub fn alias_subtitle(alias: &AliasConfig) -> String {
    match alias_kind(alias) {
        AliasKind::Url => format!("Quick link - {}", alias.target),
        AliasKind::File => format!("File - {}", alias.target),
        AliasKind::Folder => format!("Folder - {}", alias.target),
        AliasKind::Command => format!("Command - {}", alias.target),
    }
}

pub fn normalize_aliases(aliases: Vec<AliasConfig>) -> Vec<AliasConfig> {
    aliases
        .into_iter()
        .filter_map(|mut alias| {
            alias.name = alias.name.trim().to_owned();
            alias.query = alias.query.trim().to_owned();
            alias.target = alias.target.trim().to_owned();

            if alias.name.is_empty() || alias.query.is_empty() || alias.target.is_empty() {
                return None;
            }

            let kind = alias
                .kind
                .unwrap_or_else(|| infer_alias_kind(&alias.target));
            if matches!(kind, AliasKind::File | AliasKind::Folder) {
                alias.target = normalize_path_target(&alias.target);
            }

            Some(alias)
        })
        .collect()
}

fn infer_alias_kind(target: &str) -> AliasKind {
    if target.starts_with("http://") || target.starts_with("https://") {
        return AliasKind::Url;
    }

    let path = expand_home(PathBuf::from(target));
    if path.is_dir() {
        AliasKind::Folder
    } else if path.is_file()
        || target.starts_with('/')
        || target.starts_with("~/")
        || target == "~"
        || target.starts_with("./")
        || target.starts_with("../")
    {
        AliasKind::File
    } else {
        AliasKind::Command
    }
}

fn normalize_path_target(target: &str) -> String {
    expand_home(PathBuf::from(target)).display().to_string()
}

fn expand_home(path: PathBuf) -> PathBuf {
    let Some(path_str) = path.to_str() else {
        return path;
    };

    if path_str == "~" {
        return dirs::home_dir().unwrap_or(path);
    }

    if let Some(rest) = path_str.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }

    path
}
