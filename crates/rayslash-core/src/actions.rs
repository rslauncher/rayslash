use std::{
    ffi::OsString,
    io,
    path::Path,
    process::{Child, Command, Stdio},
};

use crate::apps::DesktopApp;
use crate::config::{AliasConfig, AliasKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
}

pub fn open_project_folder_command(path: &Path) -> CommandSpec {
    CommandSpec {
        program: OsString::from("xdg-open"),
        args: vec![path.as_os_str().to_owned()],
    }
}

pub fn open_project_folder(path: &Path) -> io::Result<Child> {
    let command = open_project_folder_command(path);
    spawn_command(&command)
}

pub fn open_project_in_vscode_command(path: &Path) -> CommandSpec {
    open_project_in_editor_command(path, "code")
}

pub fn open_project_in_editor_command(path: &Path, editor_command: &str) -> CommandSpec {
    let mut command = parse_action_command(editor_command).unwrap_or_else(|| CommandSpec {
        program: OsString::from(editor_command.trim()),
        args: Vec::new(),
    });

    if command.program == "xdg-terminal-exec" {
        return command;
    }

    command.args.push(path.as_os_str().to_owned());
    command
}

pub fn open_project_in_vscode(path: &Path) -> io::Result<Child> {
    open_project_in_editor(path, "code")
}

pub fn open_project_in_editor(path: &Path, editor_command: &str) -> io::Result<Child> {
    let command = open_project_in_editor_command(path, editor_command);
    if command.program == "xdg-terminal-exec" {
        spawn_command_in_dir(&command, path)
    } else {
        spawn_command(&command)
    }
}

pub fn launch_app(command: &CommandSpec) -> io::Result<Child> {
    spawn_command(command)
}

pub fn launch_alias(alias: &AliasConfig) -> io::Result<Child> {
    match crate::aliases::alias_kind(alias) {
        AliasKind::Url | AliasKind::File | AliasKind::Folder => {
            spawn_command(&open_target_command(&alias.target))
        }
        AliasKind::Command => {
            let command = parse_action_command(&alias.target).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "alias command is empty or invalid",
                )
            })?;
            spawn_command(&command)
        }
    }
}

pub fn open_url(url: &str) -> io::Result<Child> {
    spawn_command(&open_target_command(url))
}

pub fn open_default_web_search(query: &str, apps: &[DesktopApp]) -> io::Result<Child> {
    let command = default_web_search_command(query, apps)?;
    spawn_command(&command)
}

pub fn default_web_search_command(query: &str, apps: &[DesktopApp]) -> io::Result<CommandSpec> {
    let query = query.trim();
    if query.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "web search query is empty",
        ));
    }

    let desktop_id = default_web_browser_desktop_id()?;
    if let Some(app) = apps.iter().find(|app| app.id == desktop_id) {
        let mut command = app.command.clone();
        command.args.push(OsString::from(query));
        return Ok(command);
    }

    Ok(CommandSpec {
        program: OsString::from("gio"),
        args: vec![
            OsString::from("launch"),
            OsString::from(desktop_id),
            OsString::from(query),
        ],
    })
}

pub fn default_web_browser_desktop_id() -> io::Result<String> {
    let output = Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "default web browser is not configured",
        ));
    }

    let desktop_id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if desktop_id.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "default web browser is not configured",
        ));
    }

    Ok(desktop_id)
}

pub fn open_target_command(target: &str) -> CommandSpec {
    CommandSpec {
        program: OsString::from("xdg-open"),
        args: vec![OsString::from(target)],
    }
}

fn spawn_command(command: &CommandSpec) -> io::Result<Child> {
    command_builder(command).spawn()
}

fn spawn_command_in_dir(command: &CommandSpec, dir: &Path) -> io::Result<Child> {
    command_builder(command).current_dir(dir).spawn()
}

fn command_builder(command: &CommandSpec) -> Command {
    let mut builder = Command::new(&command.program);
    builder
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    builder
}

pub fn parse_action_command(command: &str) -> Option<CommandSpec> {
    let mut parts = tokenize_action_command(command)?;
    let program = parts.next()?;

    Some(CommandSpec {
        program: OsString::from(program),
        args: parts.map(OsString::from).collect(),
    })
}

fn tokenize_action_command(command: &str) -> Option<impl Iterator<Item = String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_current = false;
    let mut chars = command.trim().chars();

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

    Some(args.into_iter())
}
