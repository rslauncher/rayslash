use std::{
    ffi::OsString,
    io,
    path::Path,
    process::{Child, Command, Stdio},
};

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
    if editor_command.trim() == "xdg-terminal-exec" {
        return CommandSpec {
            program: OsString::from("xdg-terminal-exec"),
            args: Vec::new(),
        };
    }

    CommandSpec {
        program: OsString::from(editor_command.trim()),
        args: vec![path.as_os_str().to_owned()],
    }
}

pub fn open_project_in_vscode(path: &Path) -> io::Result<Child> {
    open_project_in_editor(path, "code")
}

pub fn open_project_in_editor(path: &Path, editor_command: &str) -> io::Result<Child> {
    let command = open_project_in_editor_command(path, editor_command);
    if editor_command.trim() == "xdg-terminal-exec" {
        spawn_command_in_dir(&command, path)
    } else {
        spawn_command(&command)
    }
}

pub fn launch_app(command: &CommandSpec) -> io::Result<Child> {
    spawn_command(command)
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
