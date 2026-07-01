mod paths;
mod themes;

use std::{collections::HashMap, path::PathBuf};

pub use paths::resolve_desktop_icon_in_dirs;

#[derive(Debug)]
pub(in crate::apps) struct DesktopIconResolver {
    dirs: Vec<PathBuf>,
    cache: HashMap<String, Option<PathBuf>>,
}

impl DesktopIconResolver {
    pub(in crate::apps) fn new(dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs,
            cache: HashMap::new(),
        }
    }

    pub(in crate::apps) fn resolve(&mut self, icon: &str) -> Option<PathBuf> {
        if let Some(cached) = self.cache.get(icon) {
            return cached.clone();
        }

        let resolved = resolve_desktop_icon_in_dirs(icon, &self.dirs);
        self.cache.insert(icon.to_owned(), resolved.clone());
        resolved
    }
}

pub(in crate::apps) use themes::desktop_icon_dirs;
