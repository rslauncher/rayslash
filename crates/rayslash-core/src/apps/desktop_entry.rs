use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use crate::actions::CommandSpec;

use super::DesktopApp;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DesktopEntry {
    name: Option<String>,
    generic_name: Option<String>,
    comment: Option<String>,
    exec: Option<String>,
    icon: Option<String>,
    mime_types: Vec<String>,
    try_exec: Option<String>,
    no_display: bool,
    hidden: bool,
    entry_type: Option<String>,
    only_show_in: Vec<String>,
    not_show_in: Vec<String>,
}

pub(super) fn parse_desktop_file_with_id(
    path: &Path,
    id: String,
) -> io::Result<Option<DesktopApp>> {
    let contents = fs::read_to_string(path)?;
    Ok(parse_available_desktop_entry(
        &contents,
        id,
        path.to_path_buf(),
    ))
}

pub fn parse_desktop_entry(
    contents: &str,
    id: String,
    desktop_file: PathBuf,
) -> Option<DesktopApp> {
    let entry = parse_desktop_entry_fields(contents);
    desktop_app_from_entry(entry, id, desktop_file)
}

fn parse_available_desktop_entry(
    contents: &str,
    id: String,
    desktop_file: PathBuf,
) -> Option<DesktopApp> {
    let entry = parse_desktop_entry_fields(contents);
    let app = desktop_app_from_entry(entry.clone(), id, desktop_file)?;

    if !matches_current_desktop(&entry) || !desktop_entry_is_available(&entry, &app.command) {
        return None;
    }

    Some(app)
}

fn desktop_app_from_entry(
    entry: DesktopEntry,
    id: String,
    desktop_file: PathBuf,
) -> Option<DesktopApp> {
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
        mime_types: entry.mime_types,
        icon_path: None,
        command,
        desktop_file,
    })
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

fn parse_desktop_entry_fields(contents: &str) -> DesktopEntry {
    let mut entry = DesktopEntry {
        name: None,
        generic_name: None,
        comment: None,
        exec: None,
        icon: None,
        mime_types: Vec::new(),
        try_exec: None,
        no_display: false,
        hidden: false,
        entry_type: None,
        only_show_in: Vec::new(),
        not_show_in: Vec::new(),
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
            "MimeType" => entry.mime_types = parse_desktop_list(value),
            "TryExec" => entry.try_exec = non_empty(unescape_desktop_value(value)),
            "NoDisplay" => entry.no_display = parse_desktop_bool(value),
            "Hidden" => entry.hidden = parse_desktop_bool(value),
            "Type" => entry.entry_type = non_empty(value.to_owned()),
            "OnlyShowIn" => entry.only_show_in = parse_desktop_list(value),
            "NotShowIn" => entry.not_show_in = parse_desktop_list(value),
            _ => {}
        }
    }

    entry
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

fn parse_desktop_list(value: &str) -> Vec<String> {
    value
        .split(';')
        .filter_map(|item| non_empty(unescape_desktop_value(item.trim())))
        .collect()
}

fn desktop_entry_is_available(entry: &DesktopEntry, command: &CommandSpec) -> bool {
    entry
        .try_exec
        .as_deref()
        .map(command_is_available)
        .unwrap_or_else(|| command_is_available(&command.program.to_string_lossy()))
}

fn command_is_available(command: &str) -> bool {
    let command = command.trim();
    if command.is_empty() {
        return false;
    }

    let path = Path::new(command);
    if path.is_absolute() || command.contains(std::path::MAIN_SEPARATOR) {
        return is_executable_file(path);
    }

    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| is_executable_file(&dir.join(path))))
        .unwrap_or(false)
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn matches_current_desktop(entry: &DesktopEntry) -> bool {
    let current_desktops = current_desktops();

    if !entry.only_show_in.is_empty()
        && !contains_any_desktop(&entry.only_show_in, &current_desktops)
    {
        return false;
    }

    !contains_any_desktop(&entry.not_show_in, &current_desktops)
}

fn current_desktops() -> Vec<String> {
    let mut desktops = Vec::new();

    if let Some(value) = std::env::var_os("XDG_CURRENT_DESKTOP") {
        desktops.extend(split_desktop_names(&value.to_string_lossy()));
    }

    if desktops.is_empty()
        && let Some(value) = std::env::var_os("DESKTOP_SESSION")
    {
        desktops.extend(split_desktop_names(&value.to_string_lossy()));
    }

    desktops
}

fn split_desktop_names(value: &str) -> Vec<String> {
    value
        .split([':', ';'])
        .filter_map(|desktop| non_empty(desktop.trim().to_ascii_lowercase()))
        .collect()
}

fn contains_any_desktop(entry_desktops: &[String], current_desktops: &[String]) -> bool {
    entry_desktops.iter().any(|entry_desktop| {
        let entry_desktop = entry_desktop.to_ascii_lowercase();
        current_desktops
            .iter()
            .any(|current_desktop| current_desktop == &entry_desktop)
    })
}
