use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::actions::CommandSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub command: CommandSpec,
    pub desktop_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DesktopEntry {
    name: Option<String>,
    generic_name: Option<String>,
    comment: Option<String>,
    exec: Option<String>,
    icon: Option<String>,
    no_display: bool,
    hidden: bool,
    entry_type: Option<String>,
}

pub fn discover_desktop_apps() -> Vec<DesktopApp> {
    discover_desktop_apps_in_dirs(&desktop_application_dirs())
}

pub fn discover_desktop_apps_in_dirs(dirs: &[PathBuf]) -> Vec<DesktopApp> {
    let mut seen_ids = HashSet::new();
    let mut icon_resolver = DesktopIconResolver::new(desktop_icon_dirs());
    let mut apps = Vec::new();

    for dir in dirs {
        for path in desktop_files_in_dir(dir) {
            let id = desktop_app_id(dir, &path);

            if !seen_ids.insert(id.clone()) {
                continue;
            }

            match parse_desktop_file_with_id(&path, id) {
                Ok(Some(mut app)) => {
                    app.icon_path = app
                        .icon
                        .as_deref()
                        .and_then(|icon| icon_resolver.resolve(icon));
                    apps.push(app);
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("failed to read desktop entry {}: {error}", path.display());
                }
            }
        }
    }

    apps.sort_by(app_order);
    apps
}

fn desktop_application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }

    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs
}

fn desktop_icon_dirs() -> Vec<PathBuf> {
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

fn desktop_files_in_dir(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_desktop_files(dir, &mut files);
    files.sort();
    files
}

fn collect_desktop_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_desktop_files(&path, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "desktop")
        {
            files.push(path);
        }
    }
}

fn parse_desktop_file_with_id(path: &Path, id: String) -> io::Result<Option<DesktopApp>> {
    let contents = fs::read_to_string(path)?;
    Ok(parse_desktop_entry(&contents, id, path.to_path_buf()))
}

pub fn parse_desktop_entry(
    contents: &str,
    id: String,
    desktop_file: PathBuf,
) -> Option<DesktopApp> {
    let entry = parse_desktop_entry_fields(contents);

    if entry.entry_type.as_deref() != Some("Application")
        || entry.no_display
        || entry.hidden
        || entry.name.as_deref().is_none_or(str::is_empty)
        || entry.exec.as_deref().is_none_or(str::is_empty)
    {
        return None;
    }

    let name = entry.name?;
    let exec = entry.exec?;
    let command = parse_exec_command(&exec)?;

    Some(DesktopApp {
        id,
        name,
        generic_name: entry.generic_name,
        comment: entry.comment,
        exec,
        icon: entry.icon,
        icon_path: None,
        command,
        desktop_file,
    })
}

#[derive(Debug)]
struct DesktopIconResolver {
    dirs: Vec<PathBuf>,
    cache: HashMap<String, Option<PathBuf>>,
}

impl DesktopIconResolver {
    fn new(dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs,
            cache: HashMap::new(),
        }
    }

    fn resolve(&mut self, icon: &str) -> Option<PathBuf> {
        if let Some(cached) = self.cache.get(icon) {
            return cached.clone();
        }

        let resolved = resolve_desktop_icon_in_dirs(icon, &self.dirs);
        self.cache.insert(icon.to_owned(), resolved.clone());
        resolved
    }
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

fn parse_desktop_entry_fields(contents: &str) -> DesktopEntry {
    let mut entry = DesktopEntry {
        name: None,
        generic_name: None,
        comment: None,
        exec: None,
        icon: None,
        no_display: false,
        hidden: false,
        entry_type: None,
    };
    let mut in_desktop_entry = false;

    for line in contents.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = &line[1..line.len() - 1] == "Desktop Entry";
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match key {
            "Name" => entry.name = non_empty(unescape_desktop_value(value)),
            "GenericName" => entry.generic_name = non_empty(unescape_desktop_value(value)),
            "Comment" => entry.comment = non_empty(unescape_desktop_value(value)),
            "Exec" => entry.exec = non_empty(value.to_owned()),
            "Icon" => entry.icon = non_empty(unescape_desktop_value(value)),
            "NoDisplay" => entry.no_display = parse_desktop_bool(value),
            "Hidden" => entry.hidden = parse_desktop_bool(value),
            "Type" => entry.entry_type = non_empty(value.to_owned()),
            _ => {}
        }
    }

    entry
}

pub fn parse_exec_command(exec: &str) -> Option<CommandSpec> {
    let args = tokenize_exec(exec)?;
    let mut args = args
        .into_iter()
        .filter_map(|arg| non_empty(remove_field_codes(&arg)));
    let program = args.next()?;

    Some(CommandSpec {
        program: OsString::from(program),
        args: args.map(OsString::from).collect(),
    })
}

fn tokenize_exec(exec: &str) -> Option<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_current = false;
    let mut chars = exec.chars();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_current = true;
            }
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                    has_current = true;
                } else {
                    current.push(ch);
                    has_current = true;
                }
            }
            ' ' | '\t' if !in_quotes => {
                if has_current {
                    args.push(std::mem::take(&mut current));
                    has_current = false;
                }
            }
            _ => {
                current.push(ch);
                has_current = true;
            }
        }
    }

    if in_quotes {
        return None;
    }

    if has_current {
        args.push(current);
    }

    Some(args)
}

fn remove_field_codes(arg: &str) -> String {
    let mut output = String::new();
    let mut chars = arg.chars();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }

        match chars.next() {
            Some('%') => output.push('%'),
            Some(_) => {}
            None => output.push('%'),
        }
    }

    output
}

fn unescape_desktop_value(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }

        match chars.next() {
            Some('s') => output.push(' '),
            Some('n') => output.push('\n'),
            Some('t') => output.push('\t'),
            Some('r') => output.push('\r'),
            Some('\\') => output.push('\\'),
            Some(next) => output.push(next),
            None => output.push('\\'),
        }
    }

    output
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn parse_desktop_bool(value: &str) -> bool {
    value.eq_ignore_ascii_case("true")
}

fn desktop_app_id(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "-")
}

fn app_order(a: &DesktopApp, b: &DesktopApp) -> std::cmp::Ordering {
    a.name
        .to_lowercase()
        .cmp(&b.name.to_lowercase())
        .then_with(|| a.id.cmp(&b.id))
        .then_with(|| a.desktop_file.cmp(&b.desktop_file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_desktop_entry_keeps_visible_applications() {
        let app = parse_desktop_entry(
            r#"
[Desktop Entry]
Type=Application
Name=Example Browser
GenericName=Web Browser
Comment=Browse the web
Exec=example-browser --new-window %U
Icon=example-browser
"#,
            "example.desktop".to_owned(),
            PathBuf::from("/tmp/example.desktop"),
        )
        .expect("app entry");

        assert_eq!(app.name, "Example Browser");
        assert_eq!(app.generic_name.as_deref(), Some("Web Browser"));
        assert_eq!(app.comment.as_deref(), Some("Browse the web"));
        assert_eq!(app.icon.as_deref(), Some("example-browser"));
        assert_eq!(
            app.command,
            CommandSpec {
                program: OsString::from("example-browser"),
                args: vec![OsString::from("--new-window")]
            }
        );
    }

    #[test]
    fn parse_desktop_entry_filters_hidden_and_no_display_entries() {
        assert!(
            parse_desktop_entry(
                "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nHidden=true\n",
                "hidden.desktop".to_owned(),
                PathBuf::from("/tmp/hidden.desktop"),
            )
            .is_none()
        );
        assert!(
            parse_desktop_entry(
                "[Desktop Entry]\nType=Application\nName=No Display\nExec=no-display\nNoDisplay=true\n",
                "no-display.desktop".to_owned(),
                PathBuf::from("/tmp/no-display.desktop"),
            )
            .is_none()
        );
    }

    #[test]
    fn parse_desktop_entry_filters_non_applications_and_incomplete_entries() {
        assert!(
            parse_desktop_entry(
                "[Desktop Entry]\nType=Link\nName=Site\nExec=browser\n",
                "site.desktop".to_owned(),
                PathBuf::from("/tmp/site.desktop"),
            )
            .is_none()
        );
        assert!(
            parse_desktop_entry(
                "[Desktop Entry]\nType=Application\nExec=missing-name\n",
                "missing-name.desktop".to_owned(),
                PathBuf::from("/tmp/missing-name.desktop"),
            )
            .is_none()
        );
        assert!(
            parse_desktop_entry(
                "[Desktop Entry]\nType=Application\nName=Missing Exec\n",
                "missing-exec.desktop".to_owned(),
                PathBuf::from("/tmp/missing-exec.desktop"),
            )
            .is_none()
        );
    }

    #[test]
    fn parse_exec_command_preserves_quoted_arguments_and_removes_field_codes() {
        let command = parse_exec_command(r#"sample-app --name "two words" --url=%U %f %%literal"#)
            .expect("command");

        assert_eq!(command.program, OsString::from("sample-app"));
        assert_eq!(
            command.args,
            vec![
                OsString::from("--name"),
                OsString::from("two words"),
                OsString::from("--url="),
                OsString::from("%literal"),
            ]
        );
    }

    #[test]
    fn parse_exec_command_rejects_unclosed_quotes_and_empty_commands() {
        assert!(parse_exec_command(r#"sample-app "unterminated"#).is_none());
        assert!(parse_exec_command("%U").is_none());
        assert!(parse_exec_command("").is_none());
    }

    #[test]
    fn discover_desktop_apps_in_dirs_reads_desktop_files_without_a_desktop_session() {
        let dir = unique_temp_dir("rayslash-applications");
        let icon = dir.join("app.svg");
        fs::write(
            dir.join("zeta.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Zeta\nExec=zeta %U\nIcon={}\n",
                icon.display()
            ),
        )
        .expect("write zeta desktop file");
        fs::write(&icon, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").expect("write icon");
        fs::write(
            dir.join("alpha.desktop"),
            "[Desktop Entry]\nType=Application\nName=Alpha\nExec=alpha\n",
        )
        .expect("write alpha desktop file");
        fs::write(
            dir.join("hidden.desktop"),
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nNoDisplay=true\n",
        )
        .expect("write hidden desktop file");

        let apps = discover_desktop_apps_in_dirs(std::slice::from_ref(&dir));

        assert_eq!(
            apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
            vec!["Alpha", "Zeta"]
        );
        assert_eq!(apps[1].command.program, OsString::from("zeta"));
        assert!(apps[1].command.args.is_empty());
        assert_eq!(apps[1].icon_path.as_deref(), Some(icon.as_path()));

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

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
