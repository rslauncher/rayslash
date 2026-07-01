use std::{ffi::OsString, path::PathBuf};

use rayslash_core::{
    actions::CommandSpec,
    apps::{parse_desktop_entry, parse_exec_command},
};

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
