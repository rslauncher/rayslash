//! Opt-in XDG session startup, stored in the desktop's autostart directory.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{APP_ID, atomic_write};

// Flatpak's config directory is sandbox-private. Use its existing host bridge
// for this explicit user action; pass all data as arguments, never shell code.
const HOST_SCRIPT: &str = r#"
set -eu
dir=${XDG_CONFIG_HOME:-"$HOME/.config"}/autostart
path=$dir/$2
case "$1" in
    read) if [ -e "$path" ]; then cat -- "$path"; fi ;;
    enable)
        mkdir -p -- "$dir"
        temp=$(mktemp "$dir/.rayslash.XXXXXX")
        trap 'rm -f -- "$temp"' EXIT
        printf '%s' "$3" > "$temp"
        chmod 644 "$temp"
        mv -f -- "$temp" "$path"
        ;;
    disable) rm -f -- "$path" ;;
    *) exit 2 ;;
esac
"#;

pub fn enabled() -> io::Result<bool> {
    let contents = if env::var_os("FLATPAK_ID").is_some() {
        host_operation("read", "")?
    } else {
        match fs::read_to_string(entry_path()?) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error),
        }
    };
    Ok(entry_enabled(&contents))
}

pub fn set_enabled(enable: bool) -> io::Result<()> {
    let contents = if enable {
        let executable = env::current_exe()?;
        let appimage = own_appimage(
            env::var_os("APPIMAGE").map(PathBuf::from),
            env::var_os("APPDIR").map(PathBuf::from),
            &executable,
        );
        let arguments = launch_arguments(env::var_os("FLATPAK_ID").is_some(), appimage, executable);
        desktop_entry(&arguments)?
    } else {
        String::new()
    };
    if env::var_os("FLATPAK_ID").is_some() {
        host_operation(if enable { "enable" } else { "disable" }, &contents)?;
        Ok(())
    } else {
        write_entry(&entry_path()?, enable, &contents)
    }
}

fn own_appimage(
    appimage: Option<PathBuf>,
    appdir: Option<PathBuf>,
    executable: &Path,
) -> Option<PathBuf> {
    // A native launcher started from another AppImage can inherit its parent's
    // APPIMAGE/APPDIR. Only retain the outer path for our own mounted executable.
    appimage.filter(|path| {
        path.is_absolute()
            && appdir.as_ref().is_some_and(|directory| {
                directory.is_absolute()
                    && directory != Path::new("/")
                    && executable.starts_with(directory)
            })
    })
}

fn entry_path() -> io::Result<PathBuf> {
    dirs::config_dir()
        .map(|directory| {
            directory
                .join("autostart")
                .join(format!("{APP_ID}.desktop"))
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not find the user config directory",
            )
        })
}

fn write_entry(path: &Path, enable: bool, contents: &str) -> io::Result<()> {
    if enable {
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| io::Error::other("autostart entry has no parent directory"))?,
        )?;
        atomic_write::write(path, contents)
    } else {
        match fs::remove_file(path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

fn host_operation(operation: &str, contents: &str) -> io::Result<String> {
    let output = Command::new("flatpak-spawn")
        .args([
            "--host",
            "sh",
            "-c",
            HOST_SCRIPT,
            "rayslash-autostart",
            operation,
        ])
        .arg(format!("{APP_ID}.desktop"))
        .arg(contents)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "could not {operation} login startup: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn launch_arguments(flatpak: bool, appimage: Option<PathBuf>, executable: PathBuf) -> Vec<PathBuf> {
    if flatpak {
        vec![
            "/usr/bin/flatpak".into(),
            "run".into(),
            APP_ID.into(),
            "--background".into(),
        ]
    } else {
        vec![
            appimage
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or(executable),
            "--background".into(),
        ]
    }
}

fn desktop_entry(arguments: &[PathBuf]) -> io::Result<String> {
    // GLib checks the executable's existence before expanding %% field codes.
    // An env wrapper lets percent signs in an AppImage filename be expanded as
    // an argument instead, while still executing directly without a shell.
    let percent_in_executable = arguments
        .first()
        .is_some_and(|path| path.to_string_lossy().contains('%'));
    let mut exec = arguments
        .iter()
        .map(|argument| quote_exec_argument(argument))
        .collect::<io::Result<Vec<_>>>()?
        .join(" ");
    if percent_in_executable {
        exec.insert_str(0, "\"/usr/bin/env\" ");
    }
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=Rayslash\nComment=Prepare the launcher for its keyboard shortcut\nExec={exec}\nIcon={APP_ID}\nTerminal=false\nStartupNotify=false\nX-GNOME-Autostart-enabled=true\n"
    ))
}

fn quote_exec_argument(argument: &Path) -> io::Result<String> {
    let argument = argument
        .to_str()
        .filter(|value| !value.chars().any(|c| matches!(c, '\n' | '\r' | '\0')))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "login startup requires a UTF-8 executable path without line breaks",
            )
        })?;
    let mut quoted = String::from("\"");
    for character in argument.chars() {
        match character {
            // The desktop string parser unescapes once, then Exec unquotes.
            '\\' => quoted.push_str(r"\\\\"),
            '"' | '$' | '`' => {
                quoted.push_str(r"\\");
                quoted.push(character);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    Ok(quoted)
}

fn entry_enabled(contents: &str) -> bool {
    let mut in_entry = false;
    let mut application = false;
    let mut executable = false;
    let mut disabled = false;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry && let Some((key, value)) = line.split_once('=') {
            match (key.trim(), value.trim()) {
                ("Type", "Application") => application = true,
                ("Exec", value) if !value.is_empty() => executable = true,
                ("Hidden", "true") | ("X-GNOME-Autostart-enabled", "false") => disabled = true,
                _ => {}
            }
        }
    }
    application && executable && !disabled
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn native_appimage_and_flatpak_use_persistent_login_commands() {
        let native = PathBuf::from("/usr/bin/rayslash");
        let appimage = PathBuf::from("/home/example/Launcher 100%.AppImage");
        assert_eq!(
            launch_arguments(false, None, native.clone()),
            vec![native.clone(), "--background".into()]
        );
        let arguments = launch_arguments(
            false,
            Some(appimage.clone()),
            "/tmp/.mount_launcher/usr/bin/rayslash".into(),
        );
        assert_eq!(arguments[0], appimage);
        let entry = desktop_entry(&arguments).unwrap();
        assert!(entry.contains(
            "Exec=\"/usr/bin/env\" \"/home/example/Launcher 100%%.AppImage\" \"--background\"\n"
        ));
        assert!(entry_enabled(&entry));
        assert_eq!(
            launch_arguments(false, Some(PathBuf::new()), native.clone())[0],
            native
        );
        let flatpak =
            desktop_entry(&launch_arguments(true, None, "/app/bin/rayslash".into())).unwrap();
        assert!(flatpak.contains(&format!(
            "\"/usr/bin/flatpak\" \"run\" \"{APP_ID}\" \"--background\""
        )));
        assert!(!flatpak.contains("/app/bin"));
    }

    #[test]
    fn inherited_appimage_environment_never_registers_the_parent_application() {
        let parent_image = PathBuf::from("/home/example/another-app.AppImage");
        let parent_mount = PathBuf::from("/tmp/.mount_other");
        assert_eq!(
            own_appimage(
                Some(parent_image.clone()),
                Some(parent_mount.clone()),
                Path::new("/usr/bin/rayslash")
            ),
            None
        );
        assert_eq!(
            own_appimage(
                Some(parent_image.clone()),
                None,
                Path::new("/usr/bin/rayslash")
            ),
            None
        );
        assert_eq!(
            own_appimage(
                Some(parent_image.clone()),
                Some(PathBuf::from("/")),
                Path::new("/usr/bin/rayslash")
            ),
            None
        );
        assert_eq!(
            own_appimage(
                Some(parent_image.clone()),
                Some(parent_mount),
                Path::new("/tmp/.mount_other/usr/bin/rayslash")
            ),
            Some(parent_image)
        );
    }

    #[test]
    fn desktop_disabled_entries_and_other_groups_are_respected() {
        let entry =
            desktop_entry(&launch_arguments(false, None, "/usr/bin/rayslash".into())).unwrap();
        assert!(!entry_enabled(""));
        assert!(!entry_enabled("[Other]\nType=Application\nExec=rayslash\n"));
        assert!(!entry_enabled(&format!("{entry}Hidden=true\n")));
        assert!(!entry_enabled(&entry.replace(
            "X-GNOME-Autostart-enabled=true",
            "X-GNOME-Autostart-enabled=false"
        )));
        assert!(entry_enabled(&format!("{entry}[Other]\nHidden=true\n")));
        assert!(quote_exec_argument(Path::new("/tmp/bad\npath")).is_err());
        assert_eq!(
            quote_exec_argument(Path::new("a\\b\"$`%")).unwrap(),
            r#""a\\\\b\\"\\$\\`%%""#
        );
    }

    #[test]
    fn login_entry_can_be_created_replaced_and_removed_without_affecting_other_apps() {
        let directory = test_dir();
        let path = directory
            .join("autostart")
            .join(format!("{APP_ID}.desktop"));
        let contents =
            desktop_entry(&launch_arguments(false, None, "/usr/bin/rayslash".into())).unwrap();
        write_entry(&path, true, &contents).unwrap();
        let other = path.with_file_name("other.desktop");
        fs::write(&other, "preserve").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), contents);
        write_entry(&path, true, "replacement").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "replacement");
        write_entry(&path, false, "").unwrap();
        write_entry(&path, false, "").unwrap();
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(&other).unwrap(), "preserve");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn flatpak_host_script_handles_custom_xdg_paths_and_literal_contents() {
        let directory = test_dir();
        let run = |operation: &str, contents: &str| {
            Command::new("sh")
                .args([
                    "-c",
                    HOST_SCRIPT,
                    "test",
                    operation,
                    "test.desktop",
                    contents,
                ])
                .env("XDG_CONFIG_HOME", directory.join("config with spaces"))
                .output()
                .unwrap()
        };
        let contents = "literal `commands` $(never executed) '$HOME'\n";
        assert!(run("enable", contents).status.success());
        let output = run("read", "");
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), contents);
        assert!(run("disable", "").status.success());
        assert!(run("disable", "").status.success());
        assert!(run("read", "").stdout.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn desktop_exec_survives_glib_launch_with_special_path_characters() {
        use std::{os::unix::fs::PermissionsExt, thread, time::Duration};
        if Command::new("gio").arg("version").output().is_err() {
            return;
        }
        let directory = test_dir();
        fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("launcher with spaces $ ` % \\\" ' ");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s' \"$1\" > \"$RAYSLASH_AUTOSTART_TEST_OUTPUT\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        let entry = directory.join("test.desktop");
        fs::write(
            &entry,
            desktop_entry(&launch_arguments(false, None, executable)).unwrap(),
        )
        .unwrap();
        let output_path = directory.join("argument.txt");
        let output = Command::new("gio")
            .arg("launch")
            .arg(&entry)
            .env("RAYSLASH_AUTOSTART_TEST_OUTPUT", &output_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for _ in 0..100 {
            if output_path.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(fs::read_to_string(&output_path).unwrap(), "--background");
        fs::remove_dir_all(directory).unwrap();
    }

    fn test_dir() -> PathBuf {
        env::temp_dir().join(format!(
            "rayslash-autostart-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
