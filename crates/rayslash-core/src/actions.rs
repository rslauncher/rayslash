use std::{
    ffi::OsString,
    io,
    path::Path,
    process::{Child, Command, Stdio},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    OpenPlaceholder,
}

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
    spawn_command(&command)
}

pub fn launch_app(command: &CommandSpec) -> io::Result<Child> {
    spawn_command(command)
}

fn spawn_command(command: &CommandSpec) -> io::Result<Child> {
    Command::new(&command.program)
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn open_project_folder_command_uses_xdg_open_with_project_path_argument() {
        let path = PathBuf::from("/tmp/rayslash");

        let command = open_project_folder_command(&path);

        assert_eq!(command.program, OsString::from("xdg-open"));
        assert_eq!(command.args, vec![path.into_os_string()]);
    }

    #[test]
    fn open_project_in_vscode_command_uses_code_with_project_path_argument() {
        let path = PathBuf::from("/tmp/rayslash");

        let command = open_project_in_vscode_command(&path);

        assert_eq!(command.program, OsString::from("code"));
        assert_eq!(command.args, vec![path.into_os_string()]);
    }

    #[test]
    fn open_project_in_editor_command_uses_configured_editor_with_project_path_argument() {
        let path = PathBuf::from("/tmp/rayslash");

        let command = open_project_in_editor_command(&path, "codium");

        assert_eq!(command.program, OsString::from("codium"));
        assert_eq!(command.args, vec![path.into_os_string()]);
    }

    #[test]
    fn spawn_command_runs_child_with_detached_stdio() {
        let test_binary = std::env::current_exe().expect("test binary path should be available");
        let command = CommandSpec {
            program: test_binary.into_os_string(),
            args: vec![
                OsString::from("--exact"),
                OsString::from("actions::tests::stdio_probe_child"),
                OsString::from("--nocapture"),
            ],
        };

        let mut child = spawn_command(&command).expect("test child should spawn");
        let status = child.wait().expect("test child should exit");

        assert!(status.success());
    }

    #[test]
    fn stdio_probe_child() {
        println!("stdout from child should be discarded by spawn_command");
        eprintln!("stderr from child should be discarded by spawn_command");
    }
}
