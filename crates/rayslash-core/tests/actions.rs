mod fixtures;

use std::{ffi::OsString, path::PathBuf};

use rayslash_core::actions::{self, CommandSpec};

#[test]
fn open_project_folder_command_uses_xdg_open_with_project_path_argument() {
    let path = PathBuf::from("/tmp/rayslash");

    let command = actions::open_project_folder_command(&path);

    assert_eq!(command.program, OsString::from("xdg-open"));
    assert_eq!(command.args, vec![path.into_os_string()]);
}

#[test]
fn open_target_command_uses_xdg_open_with_target_argument() {
    let command = actions::open_target_command("https://github.com");

    assert_eq!(command.program, OsString::from("xdg-open"));
    assert_eq!(command.args, vec![OsString::from("https://github.com")]);
}

#[test]
fn default_web_search_command_uses_firefox_search_mode_when_supported() {
    let mut browser = fixtures::app("zen.desktop", "Zen Browser");
    browser.command.program = OsString::from("zen-browser");

    let command =
        actions::default_web_search_command_for_app("manhattan", "zen.desktop", Some(&browser))
            .expect("browser search command");

    assert_eq!(command.program, OsString::from("zen-browser"));
    assert_eq!(
        command.args,
        vec![OsString::from("--search"), OsString::from("manhattan")]
    );
}

#[test]
fn default_web_search_command_uses_url_for_chromium_browsers() {
    let mut browser = fixtures::app("google-chrome.desktop", "Google Chrome");
    browser.command.program = OsString::from("google-chrome-stable");
    browser.command.args = vec![OsString::from("--new-window")];

    let command = actions::default_web_search_command_for_app(
        "rust slint",
        "google-chrome.desktop",
        Some(&browser),
    )
    .expect("browser search command");

    assert_eq!(command.program, OsString::from("xdg-open"));
    assert_eq!(
        command.args,
        vec![OsString::from("https://www.google.com/search?q=rust+slint")]
    );
}

#[test]
fn parse_action_command_preserves_alias_command_arguments() {
    let command =
        actions::parse_action_command(r#"notify-send "Hello world""#).expect("command spec");

    assert_eq!(command.program, OsString::from("notify-send"));
    assert_eq!(command.args, vec![OsString::from("Hello world")]);
}

#[test]
fn open_project_in_vscode_command_uses_code_with_project_path_argument() {
    let path = PathBuf::from("/tmp/rayslash");

    let command = actions::open_project_in_vscode_command(&path);

    assert_eq!(command.program, OsString::from("code"));
    assert_eq!(command.args, vec![path.into_os_string()]);
}

#[test]
fn open_project_in_editor_command_uses_configured_editor_with_project_path_argument() {
    let path = PathBuf::from("/tmp/rayslash");

    let command = actions::open_project_in_editor_command(&path, "codium");

    assert_eq!(command.program, OsString::from("codium"));
    assert_eq!(command.args, vec![path.into_os_string()]);
}

#[test]
fn open_project_in_editor_command_preserves_configured_arguments_before_path() {
    let path = PathBuf::from("/tmp/rayslash");

    let command = actions::open_project_in_editor_command(&path, "code --reuse-window");

    assert_eq!(command.program, OsString::from("code"));
    assert_eq!(
        command.args,
        vec![OsString::from("--reuse-window"), path.into_os_string()]
    );
}

#[test]
fn open_project_in_editor_command_preserves_quoted_configured_arguments() {
    let path = PathBuf::from("/tmp/rayslash");

    let command = actions::open_project_in_editor_command(&path, r#"editor "--profile=Work Dev""#);

    assert_eq!(command.program, OsString::from("editor"));
    assert_eq!(
        command.args,
        vec![OsString::from("--profile=Work Dev"), path.into_os_string()]
    );
}

#[test]
fn open_project_in_editor_command_uses_terminal_default_without_path_argument() {
    let path = PathBuf::from("/tmp/rayslash");

    let command = actions::open_project_in_editor_command(&path, "xdg-terminal-exec");

    assert_eq!(command.program, OsString::from("xdg-terminal-exec"));
    assert!(command.args.is_empty());
}

#[test]
fn open_project_in_editor_command_preserves_terminal_arguments_without_path_argument() {
    let path = PathBuf::from("/tmp/rayslash");

    let command = actions::open_project_in_editor_command(&path, "xdg-terminal-exec --wait");

    assert_eq!(command.program, OsString::from("xdg-terminal-exec"));
    assert_eq!(command.args, vec![OsString::from("--wait")]);
}

#[test]
fn launch_app_runs_child_with_detached_stdio() {
    let test_binary = std::env::current_exe().expect("test binary path should be available");
    let command = CommandSpec {
        program: test_binary.into_os_string(),
        args: vec![
            OsString::from("--exact"),
            OsString::from("stdio_probe_child"),
            OsString::from("--nocapture"),
        ],
    };

    let mut child = actions::launch_app(&command).expect("test child should spawn");
    let status = child.wait().expect("test child should exit");

    assert!(status.success());
}

#[test]
fn stdio_probe_child() {
    println!("stdout from child should be discarded by launch_app");
    eprintln!("stderr from child should be discarded by launch_app");
}
