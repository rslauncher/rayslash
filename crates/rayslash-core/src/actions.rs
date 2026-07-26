use std::{
    ffi::OsString,
    io,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use crate::search::ModuleAction;
use crate::{APP_ID, APP_NAME, apps::DesktopApp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    let outcome = if dbus_activatable && !running_in_flatpak() {
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
    const CACHE_TTL: Duration = Duration::from_secs(30);
    static CACHE: OnceLock<Mutex<Option<(Instant, String)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Some((cached_at, desktop_id)) = cache
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        && cached_at.elapsed() < CACHE_TTL
    {
        return Ok(desktop_id.clone());
    }
    let output = command_builder(&CommandSpec {
        program: OsString::from("xdg-settings"),
        args: vec![OsString::from("get"), OsString::from("default-web-browser")],
    })
    .stdout(Stdio::piped())
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

    *cache.lock().unwrap_or_else(|error| error.into_inner()) =
        Some((Instant::now(), desktop_id.clone()));
    Ok(desktop_id)
}

pub fn open_target_command(target: &str) -> CommandSpec {
    CommandSpec {
        program: OsString::from("xdg-open"),
        args: vec![OsString::from(target)],
    }
}

pub fn run_module_action(action: &ModuleAction) -> io::Result<()> {
    let command = match action {
        ModuleAction::OpenUrl(url) => open_target_command(url),
        ModuleAction::OpenPath(path) => CommandSpec {
            program: OsString::from("xdg-open"),
            args: vec![path.as_os_str().to_owned()],
        },
        ModuleAction::Notify { title, body } => notification_command(title, body),
        ModuleAction::RunApprovedCommand(arguments) => command_from_arguments(arguments)?,
        ModuleAction::ScheduleNotification { title, body, .. } => notification_command(title, body),
        ModuleAction::ScheduleCommand { command, .. } => command_from_arguments(command)?,
        ModuleAction::CopyText(_) | ModuleAction::ShowMessage(_) | ModuleAction::None => {
            return Ok(());
        }
    };
    let delay = match action {
        ModuleAction::ScheduleNotification { delay, .. }
        | ModuleAction::ScheduleCommand { delay, .. } => Duration::from_secs(*delay),
        _ => Duration::ZERO,
    };
    schedule_command(command, delay)
}

fn notification_command(title: &str, body: &str) -> CommandSpec {
    let title = notification_summary(title);
    CommandSpec {
        program: OsString::from("notify-send"),
        args: vec![
            OsString::from(format!("--app-name={APP_NAME}")),
            OsString::from(format!("--icon={APP_ID}")),
            OsString::from(format!("--hint=string:desktop-entry:{APP_ID}")),
            OsString::from(title),
            OsString::from(body),
        ],
    }
}

fn notification_summary(summary: &str) -> &str {
    if summary.eq_ignore_ascii_case("rayslash timer") {
        "Timer finished"
    } else if summary.eq_ignore_ascii_case("rayslash reminder") {
        "Reminder"
    } else {
        summary
    }
}

fn command_from_arguments(arguments: &[String]) -> io::Result<CommandSpec> {
    let (program, args) = arguments
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "module command is empty"))?;
    if program.is_empty() || arguments.iter().any(|value| value.contains('\0')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "module command contains an invalid argument",
        ));
    }
    Ok(CommandSpec {
        program: OsString::from(program),
        args: args.iter().map(OsString::from).collect(),
    })
}

fn schedule_command(command: CommandSpec, delay: Duration) -> io::Result<()> {
    if delay.is_zero() {
        spawn_and_reap(command)
    } else {
        thread::spawn(move || {
            thread::sleep(delay);
            if let Err(error) = spawn_and_reap(command) {
                eprintln!("failed to run scheduled rayslash action: {error}");
            }
        });
        Ok(())
    }
}

fn spawn_and_reap(command: CommandSpec) -> io::Result<()> {
    let mut child = spawn_command(&command)?;
    thread::spawn(move || match child.wait() {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!(
            "rayslash action `{}` exited with status {status}",
            command_display(&command)
        ),
        Err(error) => eprintln!(
            "failed to reap rayslash action `{}`: {error}",
            command_display(&command)
        ),
    });
    Ok(())
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
    let mut builder = command_builder_for_dir(command, Some(dir));
    if !running_in_flatpak() {
        builder.current_dir(dir);
    }
    builder.spawn()
}

fn command_builder(command: &CommandSpec) -> Command {
    command_builder_for_dir(command, None)
}

fn command_builder_for_dir(command: &CommandSpec, dir: Option<&Path>) -> Command {
    let mut builder = if running_in_flatpak() {
        let mut builder = Command::new("flatpak-spawn");
        builder.arg("--host");
        if let Some(dir) = dir {
            builder.arg(format!("--directory={}", dir.display()));
        }
        builder.arg(&command.program);
        builder.args(command.args.iter().map(host_visible_argument));
        builder
    } else {
        let mut builder = Command::new(&command.program);
        builder.args(&command.args);
        builder
    };
    builder
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    builder
}

fn running_in_flatpak() -> bool {
    std::env::var_os("FLATPAK_ID").is_some()
}

fn host_visible_argument(argument: &OsString) -> OsString {
    let path = Path::new(argument);
    path.strip_prefix("/run/host")
        .ok()
        .map(|path| Path::new("/").join(path).into_os_string())
        .unwrap_or_else(|| argument.clone())
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
    if std::env::var("XDG_SESSION_TYPE")
        .ok()
        .is_some_and(|session| session.eq_ignore_ascii_case("wayland"))
    {
        return false;
    }
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

    let class_targets = dedup_targets(class_targets);
    let app_name = app_name.trim();
    let output = command_builder(&CommandSpec {
        program: OsString::from("wmctrl"),
        args: vec![OsString::from("-lx")],
    })
    .stdout(Stdio::piped())
    .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let windows = String::from_utf8_lossy(&output.stdout);
    let window_id = windows.lines().find_map(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let id = *fields.first()?;
        let class = fields.get(3).copied().unwrap_or_default();
        let title = fields.get(4..).unwrap_or_default().join(" ");
        (class_targets
            .iter()
            .any(|target| class.eq_ignore_ascii_case(target) || class_ends_with(class, target))
            || (!app_name.is_empty() && title.eq_ignore_ascii_case(app_name)))
        .then_some(id)
    });
    window_id.is_some_and(|id| command_status_success("wmctrl", ["-ia", id]))
}

fn class_ends_with(class: &str, target: &str) -> bool {
    class
        .rsplit_once('.')
        .map_or(class, |(_instance, class)| class)
        .eq_ignore_ascii_case(target.trim_end_matches(".desktop"))
}

fn command_status_success<const N: usize>(program: &str, args: [&str; N]) -> bool {
    command_builder(&CommandSpec {
        program: OsString::from(program),
        args: args.into_iter().map(OsString::from).collect(),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_use_the_rayslash_desktop_identity() {
        let command = notification_command("Timer finished", "Take a break");

        assert_eq!(command.program, OsString::from("notify-send"));
        assert_eq!(
            command.args,
            vec![
                OsString::from(format!("--app-name={APP_NAME}")),
                OsString::from(format!("--icon={APP_ID}")),
                OsString::from(format!("--hint=string:desktop-entry:{APP_ID}")),
                OsString::from("Timer finished"),
                OsString::from("Take a break"),
            ]
        );
    }

    #[test]
    fn legacy_timer_notification_summaries_are_normalized() {
        let timer = notification_command("rayslash timer", "Take a break");
        let reminder = notification_command("rayslash reminder", "Take a break");

        assert_eq!(timer.args[3], OsString::from("Timer finished"));
        assert_eq!(reminder.args[3], OsString::from("Reminder"));
    }
}
