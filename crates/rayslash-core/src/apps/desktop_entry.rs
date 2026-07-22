use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use crate::actions::CommandSpec;

use super::{DesktopAction, DesktopApp};

#[derive(Debug, Clone, PartialEq, Eq)]
struct DesktopEntry {
    name: Option<String>,
    localized_names: Vec<LocalizedValue>,
    generic_name: Option<String>,
    localized_generic_names: Vec<LocalizedValue>,
    comment: Option<String>,
    localized_comments: Vec<LocalizedValue>,
    exec: Option<String>,
    icon: Option<String>,
    mime_types: Vec<String>,
    categories: Vec<String>,
    keywords: Vec<String>,
    try_exec: Option<String>,
    no_display: bool,
    hidden: bool,
    entry_type: Option<String>,
    only_show_in: Vec<String>,
    not_show_in: Vec<String>,
    dbus_activatable: bool,
    startup_wm_class: Option<String>,
    action_ids: Vec<String>,
    actions: Vec<DesktopActionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DesktopActionEntry {
    id: String,
    name: Option<String>,
    localized_names: Vec<LocalizedValue>,
    exec: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalizedValue {
    locale: String,
    value: String,
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

    if !matches_current_desktop(&entry)
        || std::env::var_os("FLATPAK_ID").is_none()
            && !desktop_entry_is_available(&entry, &app.command)
    {
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
        || (!entry.dbus_activatable && entry.exec.as_deref().is_none_or(str::is_empty))
    {
        return None;
    }

    let locale_preferences = locale_preferences();
    let name = localized_value(
        entry.name.as_deref(),
        &entry.localized_names,
        &locale_preferences,
    )?;
    let generic_name = localized_value(
        entry.generic_name.as_deref(),
        &entry.localized_generic_names,
        &locale_preferences,
    );
    let comment = localized_value(
        entry.comment.as_deref(),
        &entry.localized_comments,
        &locale_preferences,
    );
    let exec = entry.exec.clone().unwrap_or_default();
    let command = if entry.dbus_activatable && std::env::var_os("FLATPAK_ID").is_none() {
        desktop_file_launch_command(&desktop_file)
    } else {
        parse_exec_command(&exec).unwrap_or_else(|| desktop_file_launch_command(&desktop_file))
    };

    Some(DesktopApp {
        id,
        name,
        localized_names: localized_strings(&entry.localized_names),
        generic_name,
        comment,
        exec,
        icon: entry.icon,
        mime_types: entry.mime_types,
        categories: entry.categories,
        keywords: entry.keywords,
        actions: desktop_actions(entry.actions, &locale_preferences),
        dbus_activatable: entry.dbus_activatable,
        startup_wm_class: entry.startup_wm_class,
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
        localized_names: Vec::new(),
        generic_name: None,
        localized_generic_names: Vec::new(),
        comment: None,
        localized_comments: Vec::new(),
        exec: None,
        icon: None,
        mime_types: Vec::new(),
        categories: Vec::new(),
        keywords: Vec::new(),
        try_exec: None,
        no_display: false,
        hidden: false,
        entry_type: None,
        only_show_in: Vec::new(),
        not_show_in: Vec::new(),
        dbus_activatable: false,
        startup_wm_class: None,
        action_ids: Vec::new(),
        actions: Vec::new(),
    };
    let mut action_entries = BTreeMap::<String, DesktopActionEntry>::new();
    let mut current_group = DesktopEntryGroup::None;

    for line in contents.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let group = &line[1..line.len() - 1];
            current_group = if group == "Desktop Entry" {
                DesktopEntryGroup::DesktopEntry
            } else if let Some(action_id) = group.strip_prefix("Desktop Action ") {
                DesktopEntryGroup::DesktopAction(action_id.to_owned())
            } else {
                DesktopEntryGroup::None
            };
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match &current_group {
            DesktopEntryGroup::DesktopEntry => match key {
                "Name" => entry.name = non_empty(unescape_desktop_value(value)),
                key if localized_key_locale(key, "Name").is_some() => {
                    push_localized(
                        &mut entry.localized_names,
                        localized_key_locale(key, "Name").expect("localized key"),
                        unescape_desktop_value(value),
                    );
                }
                "GenericName" => entry.generic_name = non_empty(unescape_desktop_value(value)),
                key if localized_key_locale(key, "GenericName").is_some() => {
                    push_localized(
                        &mut entry.localized_generic_names,
                        localized_key_locale(key, "GenericName").expect("localized key"),
                        unescape_desktop_value(value),
                    );
                }
                "Comment" => entry.comment = non_empty(unescape_desktop_value(value)),
                key if localized_key_locale(key, "Comment").is_some() => {
                    push_localized(
                        &mut entry.localized_comments,
                        localized_key_locale(key, "Comment").expect("localized key"),
                        unescape_desktop_value(value),
                    );
                }
                "Exec" => entry.exec = non_empty(value.to_owned()),
                "Icon" => entry.icon = non_empty(unescape_desktop_value(value)),
                "MimeType" => entry.mime_types = parse_desktop_list(value),
                "Categories" => entry.categories = parse_desktop_list(value),
                "Keywords" => entry.keywords = parse_desktop_list(value),
                key if localized_key_locale(key, "Keywords").is_some() => {
                    entry.keywords.extend(parse_desktop_list(value));
                }
                "TryExec" => entry.try_exec = non_empty(unescape_desktop_value(value)),
                "NoDisplay" => entry.no_display = parse_desktop_bool(value),
                "Hidden" => entry.hidden = parse_desktop_bool(value),
                "Type" => entry.entry_type = non_empty(value.to_owned()),
                "OnlyShowIn" => entry.only_show_in = parse_desktop_list(value),
                "NotShowIn" => entry.not_show_in = parse_desktop_list(value),
                "DBusActivatable" => entry.dbus_activatable = parse_desktop_bool(value),
                "StartupWMClass" => {
                    entry.startup_wm_class = non_empty(unescape_desktop_value(value))
                }
                "Actions" => entry.action_ids = parse_desktop_list(value),
                _ => {}
            },
            DesktopEntryGroup::DesktopAction(action_id) => {
                let action =
                    action_entries
                        .entry(action_id.clone())
                        .or_insert_with(|| DesktopActionEntry {
                            id: action_id.clone(),
                            name: None,
                            localized_names: Vec::new(),
                            exec: None,
                        });

                match key {
                    "Name" => action.name = non_empty(unescape_desktop_value(value)),
                    key if localized_key_locale(key, "Name").is_some() => {
                        push_localized(
                            &mut action.localized_names,
                            localized_key_locale(key, "Name").expect("localized key"),
                            unescape_desktop_value(value),
                        );
                    }
                    "Exec" => action.exec = non_empty(value.to_owned()),
                    _ => {}
                }
            }
            DesktopEntryGroup::None => {}
        }
    }

    entry.actions = entry
        .action_ids
        .iter()
        .filter_map(|id| action_entries.remove(id))
        .collect();

    entry
}

enum DesktopEntryGroup {
    None,
    DesktopEntry,
    DesktopAction(String),
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

fn push_localized(values: &mut Vec<LocalizedValue>, locale: String, value: String) {
    if value.is_empty()
        || values
            .iter()
            .any(|existing| existing.locale == locale && existing.value == value)
    {
        return;
    }

    values.push(LocalizedValue { locale, value });
}

fn localized_key_locale(key: &str, base: &str) -> Option<String> {
    let suffix = key.strip_prefix(base)?;
    if !suffix.starts_with('[') || !suffix.ends_with(']') {
        return None;
    }

    non_empty(suffix[1..suffix.len() - 1].replace('-', "_"))
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

fn desktop_file_launch_command(path: &Path) -> CommandSpec {
    CommandSpec {
        program: OsString::from("gio"),
        args: vec![OsString::from("launch"), path.as_os_str().to_owned()],
    }
}

fn desktop_actions(
    actions: Vec<DesktopActionEntry>,
    locale_preferences: &[String],
) -> Vec<DesktopAction> {
    actions
        .into_iter()
        .filter_map(|action| {
            let name = localized_value(
                action.name.as_deref(),
                &action.localized_names,
                locale_preferences,
            )?;
            let command = action.exec.as_deref().and_then(parse_exec_command);

            Some(DesktopAction {
                id: action.id,
                name,
                exec: action.exec,
                command,
            })
        })
        .collect()
}

fn localized_value(
    default: Option<&str>,
    localized_values: &[LocalizedValue],
    locale_preferences: &[String],
) -> Option<String> {
    for preferred in locale_preferences {
        if let Some(value) = localized_values
            .iter()
            .find(|localized| localized.locale.eq_ignore_ascii_case(preferred))
        {
            return Some(value.value.clone());
        }

        let preferred_language = preferred.split('_').next().unwrap_or(preferred);
        if let Some(value) = localized_values.iter().find(|localized| {
            localized
                .locale
                .split('_')
                .next()
                .is_some_and(|language| language.eq_ignore_ascii_case(preferred_language))
        }) {
            return Some(value.value.clone());
        }
    }

    default.map(str::to_owned)
}

fn localized_strings(localized_values: &[LocalizedValue]) -> Vec<String> {
    let mut values = Vec::new();
    for localized in localized_values {
        if !values.contains(&localized.value) {
            values.push(localized.value.clone());
        }
    }
    values
}

fn locale_preferences() -> Vec<String> {
    for key in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(value) = std::env::var_os(key) {
            let value = value.to_string_lossy();
            let locales = value
                .split([':', ';'])
                .filter_map(|locale| {
                    let locale = locale
                        .split('.')
                        .next()
                        .unwrap_or(locale)
                        .trim()
                        .replace('-', "_");
                    non_empty(locale)
                })
                .collect::<Vec<_>>();
            if !locales.is_empty() {
                return locales;
            }
        }
    }

    Vec::new()
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
