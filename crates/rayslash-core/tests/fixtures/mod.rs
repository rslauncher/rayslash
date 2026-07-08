#![allow(dead_code)]

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rayslash_core::{
    actions::CommandSpec, apps::DesktopApp, config, projects::Project, ranking::RankingState,
};

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("create temp dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn join(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.path.join(relative)
    }

    pub fn create_dir_all(&self, relative: impl AsRef<Path>) -> io::Result<PathBuf> {
        let path = self.join(relative);
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn write(
        &self,
        relative: impl AsRef<Path>,
        contents: impl AsRef<[u8]>,
    ) -> io::Result<PathBuf> {
        let path = self.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, contents)?;
        Ok(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn desktop_entry(name: &str, exec: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n")
}

pub fn write_hicolor_app_icon(dir: &TempDir, size: &str, name: &str, extension: &str) -> PathBuf {
    dir.write(
        format!("hicolor/{size}/apps/{name}.{extension}"),
        if extension.eq_ignore_ascii_case("svg") {
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>"
        } else {
            "not a real image"
        },
    )
    .expect("write hicolor app icon")
}

pub fn config_with_max_results(max_results: usize) -> config::Config {
    config::Config {
        folder_sources: Vec::new(),
        aliases: Vec::new(),
        web_searches: Vec::new(),
        providers: config::ProviderConfig::default(),
        actions: config::ActionConfig::default(),
        appearance: config::AppearanceConfig {
            max_results,
            ..config::AppearanceConfig::default()
        },
        ranking: config::RankingConfig::default(),
    }
}

pub fn ranking_with_launches(id: &str, query: &str, count: u64) -> RankingState {
    let mut ranking = RankingState::default();
    for second in 1..=count {
        ranking.record_launch_at(
            id,
            query,
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(second),
        );
    }
    ranking
}

pub fn project(path: impl Into<PathBuf>, name: &str) -> Project {
    Project {
        name: name.to_owned(),
        path: path.into(),
    }
}

pub fn app(id: &str, name: &str) -> DesktopApp {
    DesktopApp {
        id: id.to_owned(),
        name: name.to_owned(),
        localized_names: Vec::new(),
        generic_name: None,
        comment: None,
        exec: name.to_ascii_lowercase(),
        icon: None,
        mime_types: Vec::new(),
        categories: Vec::new(),
        keywords: Vec::new(),
        actions: Vec::new(),
        dbus_activatable: false,
        startup_wm_class: None,
        icon_path: None,
        command: CommandSpec {
            program: name.to_ascii_lowercase().into(),
            args: Vec::new(),
        },
        desktop_file: PathBuf::from(format!("/tmp/{id}")),
    }
}
