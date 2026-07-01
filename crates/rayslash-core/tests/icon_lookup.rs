mod fixtures;

use fixtures::TempDir;
use rayslash_core::apps::resolve_desktop_icon_in_dirs;

#[test]
fn resolve_desktop_icon_supports_absolute_paths() {
    let dir = TempDir::new("rayslash-icons-absolute");
    let icon = dir
        .write(
            "absolute.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .expect("write icon");

    assert_eq!(
        resolve_desktop_icon_in_dirs(icon.to_str().expect("utf-8 path"), &[]),
        Some(icon)
    );
}

#[test]
fn resolve_desktop_icon_checks_hicolor_app_directories() {
    let dir = TempDir::new("rayslash-icons-hicolor");
    let icon = dir
        .write(
            "hicolor/scalable/apps/example.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .expect("write icon");

    assert_eq!(
        resolve_desktop_icon_in_dirs("example", &[dir.path().to_path_buf()]),
        Some(icon)
    );
}

#[test]
fn resolve_desktop_icon_prefers_launcher_sized_hicolor_icons() {
    let dir = TempDir::new("rayslash-icons-preferred-size");
    let icon_48 = dir
        .write("hicolor/48x48/apps/example.png", "not a real png")
        .expect("write 48px icon");
    dir.write(
        "hicolor/scalable/apps/example.svg",
        "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    )
    .expect("write scalable icon");

    assert_eq!(
        resolve_desktop_icon_in_dirs("example", &[dir.path().to_path_buf()]),
        Some(icon_48)
    );
}

#[test]
fn resolve_desktop_icon_checks_theme_specific_app_directories() {
    let dir = TempDir::new("rayslash-icons-themed");
    let theme_dir = dir.create_dir_all("Papirus").expect("create theme dir");
    let icon = dir
        .write(
            "Papirus/42x42/apps/example.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .expect("write icon");

    assert_eq!(
        resolve_desktop_icon_in_dirs("example", &[theme_dir]),
        Some(icon)
    );
}

#[test]
fn resolve_desktop_icon_checks_pixmaps_style_directories() {
    let dir = TempDir::new("rayslash-icons-pixmaps");
    let icon = dir
        .write("example.png", "not a real png")
        .expect("write icon");

    assert_eq!(
        resolve_desktop_icon_in_dirs("example", &[dir.path().to_path_buf()]),
        Some(icon)
    );
    assert_eq!(
        resolve_desktop_icon_in_dirs("example.xpm", &[dir.path().to_path_buf()]),
        None
    );
}
