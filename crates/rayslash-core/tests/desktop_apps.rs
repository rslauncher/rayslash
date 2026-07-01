mod fixtures;

use std::ffi::OsString;

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

    dir.write(
        "nested/zeta.desktop",
        format!(
            "[Desktop Entry]\nType=Application\nName=Zeta\nExec=zeta --name \"two words\" %U\nIcon={}\n",
            icon.display()
        ),
    )
    .expect("write zeta desktop file");
    dir.write("alpha.desktop", desktop_entry("Alpha", "alpha"))
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

    let apps = discover_desktop_apps_in_dirs(&[dir.path().to_path_buf()]);

    assert_eq!(
        apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
        vec!["Alpha", "Zeta"]
    );
    assert_eq!(apps[1].id, "nested-zeta.desktop");
    assert_eq!(apps[1].command.program, OsString::from("zeta"));
    assert_eq!(
        apps[1].command.args,
        vec![OsString::from("--name"), OsString::from("two words")]
    );
    assert_eq!(apps[1].icon_path.as_deref(), Some(icon.as_path()));
}

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
