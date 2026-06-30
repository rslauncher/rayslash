use std::{
    ffi::OsString,
    io,
    path::Path,
    process::{Child, Command},
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

pub fn open_project_in_vscode_command(path: &Path) -> CommandSpec {
    CommandSpec {
        program: OsString::from("code"),
        args: vec![path.as_os_str().to_owned()],
    }
}

pub fn open_project_in_vscode(path: &Path) -> io::Result<Child> {
    let command = open_project_in_vscode_command(path);
    Command::new(command.program).args(command.args).spawn()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn open_project_in_vscode_command_uses_code_with_project_path_argument() {
        let path = PathBuf::from("/tmp/rayslash");

        let command = open_project_in_vscode_command(&path);

        assert_eq!(command.program, OsString::from("code"));
        assert_eq!(command.args, vec![path.into_os_string()]);
    }
}
