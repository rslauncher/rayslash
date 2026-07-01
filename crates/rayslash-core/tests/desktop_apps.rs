mod fixtures;

use std::{ffi::OsString, path::PathBuf};

use fixtures::{TempDir, desktop_entry, write_hicolor_app_icon};
use rayslash_core::apps::{discover_desktop_apps_in_dirs, resolve_desktop_icon_in_dirs};

#[test]
fn desktop_discovery_uses_fixtures_for_filtering_ids_exec_and_absolute_icons() {
    let dir = TempDir::new("rayslash-desktop-apps-fixture");
    let icon = dir
        .write(
            "icons/zeta.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .expect("write icon");
    let alpha_exec = write_executable(&dir, "bin/alpha");
    let zeta_exec = write_executable(&dir, "bin/zeta");

    dir.write(
        "nested/zeta.desktop",
        format!(
            "[Desktop Entry]\nType=Application\nName=Zeta\nExec={} --name \"two words\" %U\nIcon={}\n",
            zeta_exec.display(),
            icon.display()
        ),
    )
    .expect("write zeta desktop file");
    dir.write(
        "alpha.desktop",
        desktop_entry("Alpha", &alpha_exec.display().to_string()),
    )
    .expect("write alpha desktop file");
    dir.write(
        "hidden.desktop",
        "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nHidden=true\n",
    )
    .expect("write hidden desktop file");
    dir.write(
        "no-display.desktop",
        "[Desktop Entry]\nType=Application\nName=No Display\nExec=no-display\nNoDisplay=true\n",
    )
    .expect("write no-display desktop file");
    dir.write(
        "broken.desktop",
        "[Desktop Entry]\nType=Application\nName=Broken\n",
    )
    .expect("write broken desktop file");
    dir.write(
        "missing-exec.desktop",
        format!(
            "[Desktop Entry]\nType=Application\nName=Missing Exec Target\nExec={}\n",
            dir.join("bin/missing").display()
        ),
    )
    .expect("write missing exec desktop file");
    dir.write(
        "missing-try-exec.desktop",
        format!(
            "[Desktop Entry]\nType=Application\nName=Missing TryExec\nExec={}\nTryExec={}\n",
            alpha_exec.display(),
            dir.join("bin/missing-try-exec").display()
        ),
    )
    .expect("write missing TryExec desktop file");

    let apps = discover_desktop_apps_in_dirs(&[dir.path().to_path_buf()]);

    assert_eq!(
        apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
        vec!["Alpha", "Zeta"]
    );
    assert_eq!(apps[1].id, "nested-zeta.desktop");
    assert_eq!(apps[1].command.program, OsString::from(zeta_exec));
    assert_eq!(
        apps[1].command.args,
        vec![OsString::from("--name"), OsString::from("two words")]
    );
    assert_eq!(apps[1].icon_path.as_deref(), Some(icon.as_path()));
}

fn write_executable(dir: &TempDir, relative: &str) -> PathBuf {
    let path = dir
        .write(relative, "#!/bin/sh\n")
        .expect("write executable fixture");
    set_executable(&path);
    path
}

#[cfg(unix)]
fn set_executable(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .expect("fixture executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("fixture executable permissions");
}

#[cfg(not(unix))]
fn set_executable(_path: &PathBuf) {}

#[test]
fn icon_theme_fixture_prefers_launcher_sized_hicolor_assets() {
    let dir = TempDir::new("rayslash-icon-theme-fixture");
    let icon_48 = write_hicolor_app_icon(&dir, "48x48", "example", "png");
    write_hicolor_app_icon(&dir, "scalable", "example", "svg");

    assert_eq!(
        resolve_desktop_icon_in_dirs("example", &[dir.path().to_path_buf()]),
        Some(icon_48)
    );
}
