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
    no_display: bool,
    hidden: bool,
    entry_type: Option<String>,
}

pub(super) fn parse_desktop_file_with_id(
    path: &Path,
    id: String,
) -> io::Result<Option<DesktopApp>> {
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
