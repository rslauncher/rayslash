use std::{
    ffi::OsString,
    io,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::apps::DesktopApp;
use crate::config::{AliasConfig, AliasKind};
use crate::utility_actions::{SystemActionKind, UtilityAction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
}

pub enum LaunchOutcome {
    Spawned(Child),
    Completed,
    FocusedExisting,
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

pub fn activate_app(
    desktop_id: &str,
    app_name: &str,
    command: &CommandSpec,
    desktop_file: &Path,
    dbus_activatable: bool,
    startup_wm_class: Option<&str>,
) -> io::Result<LaunchOutcome> {
    if try_focus_existing_app_window(desktop_id, app_name, startup_wm_class) {
        return Ok(LaunchOutcome::FocusedExisting);
    }

    let outcome = if dbus_activatable {
        launch_desktop_file(desktop_file)
    } else {
        match spawn_command(command) {
            Ok(child) => Ok(LaunchOutcome::Spawned(child)),
            Err(_command_error) => launch_desktop_file(desktop_file),
        }
    }?;

    focus_app_window_after_delay(
        desktop_id.to_owned(),
        app_name.to_owned(),
        startup_wm_class.map(str::to_owned),
    );
    Ok(outcome)
}

fn launch_desktop_file(desktop_file: &Path) -> io::Result<LaunchOutcome> {
    let desktop_command = desktop_app_launch_command(desktop_file);
    match spawn_command_checked(&desktop_command)? {
        LaunchProcess::Running(child) => Ok(LaunchOutcome::Spawned(child)),
        LaunchProcess::Completed => Ok(LaunchOutcome::Completed),
    }
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

pub fn open_default_web_search(query: &str, apps: &[DesktopApp]) -> io::Result<LaunchOutcome> {
    let desktop_id = default_web_browser_desktop_id()?;
    let app = apps.iter().find(|app| app.id == desktop_id);
    let command = default_web_search_command_for_app(query, &desktop_id, app)?;
    let child = spawn_command(&command)?;

    if let Some(app) = app {
        focus_app_window_after_delay(
            app.id.clone(),
            app.name.clone(),
            app.startup_wm_class.clone(),
        );
    }

    Ok(LaunchOutcome::Spawned(child))
}

pub fn default_web_search_command(query: &str, apps: &[DesktopApp]) -> io::Result<CommandSpec> {
    let desktop_id = default_web_browser_desktop_id()?;
    let app = apps.iter().find(|app| app.id == desktop_id);

    default_web_search_command_for_app(query, &desktop_id, app)
}

pub fn default_web_search_command_for_app(
    query: &str,
    desktop_id: &str,
    app: Option<&DesktopApp>,
) -> io::Result<CommandSpec> {
    let query = query.trim();
    if query.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "web search query is empty",
        ));
    }

    if let Some(app) = app {
        let mut command = app.command.clone();
        if is_firefox_like_browser(desktop_id, &command.program) {
            command.args.push(OsString::from("--search"));
            command.args.push(OsString::from(query));
            return Ok(command);
        }
        if is_chromium_like_browser(desktop_id, &command.program) {
            return Ok(open_target_command(&format!(
                "https://www.google.com/search?q={}",
                url_encode(query)
            )));
        }
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

pub fn run_utility_action(action: &UtilityAction) -> io::Result<()> {
    match action {
        UtilityAction::System(action) => {
            schedule_command(system_action_command(action.kind), action.delay)
        }
        UtilityAction::Timer(action) => {
            schedule_command(timer_notification_command(&action.message), action.delay)
        }
    }
}

pub fn system_action_command(kind: SystemActionKind) -> CommandSpec {
    crate::utility_actions::system_action_command(kind)
}

pub fn timer_notification_command(message: &str) -> CommandSpec {
    crate::utility_actions::timer_notification_command(message)
}

fn schedule_command(command: CommandSpec, delay: Duration) -> io::Result<()> {
    if delay.is_zero() {
        spawn_command_checked(&command).map(|_| ())
    } else {
        thread::spawn(move || {
            thread::sleep(delay);
            if let Err(error) = spawn_command_checked(&command) {
                eprintln!("failed to run scheduled rayslash action: {error}");
            }
        });
        Ok(())
    }
}

fn desktop_app_launch_command(desktop_file: &Path) -> CommandSpec {
    CommandSpec {
        program: OsString::from("gio"),
        args: vec![
            OsString::from("launch"),
            desktop_file.as_os_str().to_owned(),
        ],
    }
}

fn spawn_command(command: &CommandSpec) -> io::Result<Child> {
    command_builder(command).spawn()
}

enum LaunchProcess {
    Running(Child),
    Completed,
}

fn spawn_command_checked(command: &CommandSpec) -> io::Result<LaunchProcess> {
    let mut child = spawn_command(command)?;
    let deadline = Instant::now() + Duration::from_millis(150);

    loop {
        match child.try_wait()? {
            Some(status) if status.success() => return Ok(LaunchProcess::Completed),
            Some(status) => return Err(exit_status_error(command, status)),
            None if Instant::now() >= deadline => return Ok(LaunchProcess::Running(child)),
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn exit_status_error(command: &CommandSpec, status: ExitStatus) -> io::Error {
    io::Error::other(format!(
        "`{}` exited with status {status}",
        command_display(command)
    ))
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

fn focus_app_window_after_delay(
    desktop_id: String,
    app_name: String,
    startup_wm_class: Option<String>,
) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));
        try_focus_existing_app_window(&desktop_id, &app_name, startup_wm_class.as_deref());
    });
}

fn try_focus_existing_app_window(
    desktop_id: &str,
    app_name: &str,
    startup_wm_class: Option<&str>,
) -> bool {
    let mut class_targets = Vec::new();
    if let Some(startup_wm_class) = startup_wm_class
        && !startup_wm_class.trim().is_empty()
    {
        class_targets.push(startup_wm_class.trim().to_owned());
    }

    let desktop_id = desktop_id.trim();
    if !desktop_id.is_empty() {
        class_targets.push(desktop_id.to_owned());
        if let Some(without_suffix) = desktop_id.strip_suffix(".desktop")
            && !without_suffix.is_empty()
        {
            class_targets.push(without_suffix.to_owned());
        }
    }

    for target in dedup_targets(class_targets) {
        if command_status_success("wmctrl", ["-x", "-a", target.as_str()]) {
            return true;
        }
    }

    let app_name = app_name.trim();
    !app_name.is_empty() && command_status_success("wmctrl", ["-a", app_name])
}

fn command_status_success<const N: usize>(program: &str, args: [&str; N]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn dedup_targets(targets: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for target in targets {
        if !deduped
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&target))
        {
            deduped.push(target);
        }
    }
    deduped
}

fn command_display(command: &CommandSpec) -> String {
    std::iter::once(command.program.to_string_lossy().into_owned())
        .chain(
            command
                .args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned()),
        )
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_firefox_like_browser(desktop_id: &str, program: &std::ffi::OsStr) -> bool {
    let id = desktop_id.to_ascii_lowercase();
    let program = program.to_string_lossy().to_ascii_lowercase();
    ["firefox", "librewolf", "waterfox", "icecat", "zen"]
        .iter()
        .any(|name| id.contains(name) || program.contains(name))
}

fn is_chromium_like_browser(desktop_id: &str, program: &std::ffi::OsStr) -> bool {
    let id = desktop_id.to_ascii_lowercase();
    let program = program.to_string_lossy().to_ascii_lowercase();
    [
        "chromium",
        "chrome",
        "brave",
        "vivaldi",
        "opera",
        "microsoft-edge",
        "thorium",
    ]
    .iter()
    .any(|name| id.contains(name) || program.contains(name))
}

fn url_encode(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            b' ' => encoded.push('+'),
            byte => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
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
