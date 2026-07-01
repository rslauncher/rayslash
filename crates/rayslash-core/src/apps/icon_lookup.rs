use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug)]
pub(super) struct DesktopIconResolver {
    dirs: Vec<PathBuf>,
    cache: HashMap<String, Option<PathBuf>>,
}

impl DesktopIconResolver {
    pub(super) fn new(dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs,
            cache: HashMap::new(),
        }
    }

    pub(super) fn resolve(&mut self, icon: &str) -> Option<PathBuf> {
        if let Some(cached) = self.cache.get(icon) {
            return cached.clone();
        }

        let resolved = resolve_desktop_icon_in_dirs(icon, &self.dirs);
        self.cache.insert(icon.to_owned(), resolved.clone());
        resolved
    }
}

pub(super) fn desktop_icon_dirs() -> Vec<PathBuf> {
    let base_dirs = desktop_icon_base_dirs();
    let mut dirs = Vec::new();
    let mut seen_paths = HashSet::new();

    for theme_name in preferred_icon_theme_names(&base_dirs) {
        add_icon_theme_dir(
            &theme_name,
            &base_dirs,
            &mut dirs,
            &mut seen_paths,
            &mut HashSet::new(),
        );
    }

    for base_dir in base_dirs {
        push_unique_path(&mut dirs, &mut seen_paths, base_dir);
    }

    push_unique_path(
        &mut dirs,
        &mut seen_paths,
        PathBuf::from("/usr/share/pixmaps"),
    );

    dirs
}

pub fn resolve_desktop_icon_in_dirs(icon: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let icon = icon.trim();

    if icon.is_empty() {
        return None;
    }

    let icon_path = Path::new(icon);
    if icon_path.is_absolute() {
        return supported_existing_icon(icon_path);
    }

    if icon_path.components().count() > 1 {
        return None;
    }

    for dir in dirs {
        if let Some(path) = resolve_desktop_icon_in_dir(icon, dir) {
            return Some(path);
        }
    }

    None
}

fn desktop_icon_base_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen_paths = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        push_unique_path(&mut dirs, &mut seen_paths, home.join(".local/share/icons"));
        push_unique_path(&mut dirs, &mut seen_paths, home.join(".icons"));
    }

    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_else(|| {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        });

    for data_dir in data_dirs {
        push_unique_path(&mut dirs, &mut seen_paths, data_dir.join("icons"));
    }

    dirs
}

fn preferred_icon_theme_names(base_dirs: &[PathBuf]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen_names = HashSet::new();

    for name in configured_icon_theme_names() {
        push_unique_name(&mut names, &mut seen_names, name);
    }

    for name in [
        "Papirus",
        "Papirus-Dark",
        "Papirus-Light",
        "breeze-dark",
        "breeze",
        "Yaru",
        "Adwaita",
        "hicolor",
    ] {
        if icon_theme_exists(name, base_dirs) {
            push_unique_name(&mut names, &mut seen_names, name.to_owned());
        }
    }

    let mut discovered = base_dirs
        .iter()
        .filter_map(|base_dir| fs::read_dir(base_dir).ok())
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| {
            let path = entry.path();
            path.join("index.theme")
                .is_file()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    discovered.sort_by_key(|name| name.to_ascii_lowercase());

    for name in discovered {
        push_unique_name(&mut names, &mut seen_names, name);
    }

    names
}

fn configured_icon_theme_names() -> Vec<String> {
    let mut names = Vec::new();
    let mut seen_names = HashSet::new();

    if let Some(name) = icon_theme_from_gsettings() {
        push_unique_name(&mut names, &mut seen_names, name);
    }

    if let Some(home) = dirs::home_dir() {
        for path in [
            home.join(".config/gtk-4.0/settings.ini"),
            home.join(".config/gtk-3.0/settings.ini"),
        ] {
            if let Some(name) = icon_theme_from_ini(&path, "Settings", "gtk-icon-theme-name") {
                push_unique_name(&mut names, &mut seen_names, name);
            }
        }

        if let Some(name) = icon_theme_from_ini(&home.join(".config/kdeglobals"), "Icons", "Theme")
        {
            push_unique_name(&mut names, &mut seen_names, name);
        }
    }

    names
}

fn icon_theme_from_gsettings() -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    non_empty(unquote_config_value(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

fn icon_theme_from_ini(path: &Path, section: &str, key: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let value = ini_value(&contents, section, key)?;
    non_empty(unquote_config_value(value.trim()))
}

fn ini_value<'a>(contents: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut in_section = false;

    for line in contents.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_section = &line[1..line.len() - 1] == section;
            continue;
        }

        if !in_section {
            continue;
        }

        let Some((candidate_key, value)) = line.split_once('=') else {
            continue;
        };

        if candidate_key.trim() == key {
            return Some(value);
        }
    }

    None
}

fn unquote_config_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('\'')
        .trim_matches('"')
        .trim()
        .to_owned()
}

fn add_icon_theme_dir(
    theme_name: &str,
    base_dirs: &[PathBuf],
    dirs: &mut Vec<PathBuf>,
    seen_paths: &mut HashSet<PathBuf>,
    seen_names: &mut HashSet<String>,
) {
    if !seen_names.insert(theme_name.to_ascii_lowercase()) {
        return;
    }

    for theme_dir in icon_theme_paths(theme_name, base_dirs) {
        push_unique_path(dirs, seen_paths, theme_dir.clone());

        for inherited_theme in icon_theme_inherits(&theme_dir) {
            add_icon_theme_dir(&inherited_theme, base_dirs, dirs, seen_paths, seen_names);
        }
    }
}

fn icon_theme_paths(theme_name: &str, base_dirs: &[PathBuf]) -> Vec<PathBuf> {
    base_dirs
        .iter()
        .map(|base_dir| base_dir.join(theme_name))
        .filter(|path| path.join("index.theme").is_file())
        .collect()
}

fn icon_theme_exists(theme_name: &str, base_dirs: &[PathBuf]) -> bool {
    icon_theme_paths(theme_name, base_dirs)
        .into_iter()
        .next()
        .is_some()
}

fn icon_theme_inherits(theme_dir: &Path) -> Vec<String> {
    let Ok(contents) = fs::read_to_string(theme_dir.join("index.theme")) else {
        return Vec::new();
    };

    ini_value(&contents, "Icon Theme", "Inherits")
        .map(|value| {
            value
                .split(',')
                .filter_map(|name| non_empty(name.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn push_unique_path(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if seen.insert(path.clone()) {
        paths.push(path);
    }
}

fn push_unique_name(names: &mut Vec<String>, seen: &mut HashSet<String>, name: String) {
    if name.is_empty() {
        return;
    }

    if seen.insert(name.to_ascii_lowercase()) {
        names.push(name);
    }
}

fn resolve_desktop_icon_in_dir(icon: &str, dir: &Path) -> Option<PathBuf> {
    let names = icon_candidate_file_names(icon);

    for name in &names {
        if let Some(path) = supported_existing_icon(&dir.join(name)) {
            return Some(path);
        }
    }

    for theme_root in icon_theme_roots(dir) {
        for relative_dir in icon_theme_app_dirs() {
            for name in &names {
                if let Some(path) =
                    supported_existing_icon(&theme_root.join(&relative_dir).join(name))
                {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn icon_theme_roots(dir: &Path) -> Vec<PathBuf> {
    let mut roots = vec![dir.to_path_buf()];

    if dir.file_name().and_then(|name| name.to_str()) != Some("hicolor") {
        roots.push(dir.join("hicolor"));
    }

    roots
}

fn icon_theme_app_dirs() -> Vec<PathBuf> {
    [
        "42x42/apps",
        "48x48/apps",
        "32x32/apps",
        "24x24/apps",
        "22x22/apps",
        "64x64/apps",
        "84x84/apps",
        "96x96/apps",
        "128x128/apps",
        "256x256/apps",
        "512x512/apps",
        "16x16/apps",
        "scalable/apps",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn icon_candidate_file_names(icon: &str) -> Vec<String> {
    let path = Path::new(icon);

    if path.extension().is_some_and(is_supported_icon_extension) {
        return vec![icon.to_owned()];
    }

    ["svg", "png", "jpg", "jpeg"]
        .into_iter()
        .map(|extension| format!("{icon}.{extension}"))
        .collect()
}

fn supported_existing_icon(path: &Path) -> Option<PathBuf> {
    path.extension()
        .filter(|extension| is_supported_icon_extension(extension))
        .and_then(|_| path.is_file().then(|| path.to_path_buf()))
}

fn is_supported_icon_extension(extension: &std::ffi::OsStr) -> bool {
    extension.to_str().is_some_and(|extension| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "svg" | "png" | "jpg" | "jpeg"
        )
    })
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_desktop_icon_supports_absolute_paths() {
        let dir = unique_temp_dir("rayslash-icons-absolute");
        let icon = dir.join("absolute.svg");
        fs::write(&icon, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").expect("write icon");

        assert_eq!(
            resolve_desktop_icon_in_dirs(icon.to_str().expect("utf-8 path"), &[]),
            Some(icon.clone())
        );

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn resolve_desktop_icon_checks_hicolor_app_directories() {
        let dir = unique_temp_dir("rayslash-icons-hicolor");
        let icon_dir = dir.join("hicolor/scalable/apps");
        fs::create_dir_all(&icon_dir).expect("create icon dir");
        let icon = icon_dir.join("example.svg");
        fs::write(&icon, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").expect("write icon");

        assert_eq!(
            resolve_desktop_icon_in_dirs("example", std::slice::from_ref(&dir)),
            Some(icon.clone())
        );

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn resolve_desktop_icon_prefers_launcher_sized_hicolor_icons() {
        let dir = unique_temp_dir("rayslash-icons-preferred-size");
        let icon_48_dir = dir.join("hicolor/48x48/apps");
        let icon_scalable_dir = dir.join("hicolor/scalable/apps");
        fs::create_dir_all(&icon_48_dir).expect("create 48px icon dir");
        fs::create_dir_all(&icon_scalable_dir).expect("create scalable icon dir");
        let icon_48 = icon_48_dir.join("example.png");
        let icon_scalable = icon_scalable_dir.join("example.svg");
        fs::write(&icon_48, "not a real png").expect("write 48px icon");
        fs::write(
            &icon_scalable,
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .expect("write scalable icon");

        assert_eq!(
            resolve_desktop_icon_in_dirs("example", std::slice::from_ref(&dir)),
            Some(icon_48.clone())
        );

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn resolve_desktop_icon_checks_theme_specific_app_directories() {
        let dir = unique_temp_dir("rayslash-icons-themed");
        let theme_dir = dir.join("Papirus");
        let icon_dir = theme_dir.join("42x42/apps");
        fs::create_dir_all(&icon_dir).expect("create themed icon dir");
        let icon = icon_dir.join("example.svg");
        fs::write(&icon, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").expect("write icon");

        assert_eq!(
            resolve_desktop_icon_in_dirs("example", std::slice::from_ref(&theme_dir)),
            Some(icon.clone())
        );

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn resolve_desktop_icon_checks_pixmaps_style_directories() {
        let dir = unique_temp_dir("rayslash-icons-pixmaps");
        let icon = dir.join("example.png");
        fs::write(&icon, "not a real png").expect("write icon");

        assert_eq!(
            resolve_desktop_icon_in_dirs("example", std::slice::from_ref(&dir)),
            Some(icon.clone())
        );
        assert_eq!(
            resolve_desktop_icon_in_dirs("example.xpm", std::slice::from_ref(&dir)),
            None
        );

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("create temp dir");
        path
    }
}
