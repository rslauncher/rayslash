use std::{
    collections::HashSet,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
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
    let mut apps = Vec::new();

    for dir in dirs {
        for path in desktop_files_in_dir(dir) {
            let id = desktop_app_id(dir, &path);

            if !seen_ids.insert(id.clone()) {
                continue;
            }

            match parse_desktop_file_with_id(&path, id) {
                Ok(Some(app)) => apps.push(app),
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
        command,
        desktop_file,
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
        fs::write(
            dir.join("zeta.desktop"),
            "[Desktop Entry]\nType=Application\nName=Zeta\nExec=zeta %U\n",
        )
        .expect("write zeta desktop file");
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
