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
fn resolve_desktop_icon_supports_absolute_extensionless_paths() {
    let dir = TempDir::new("rayslash-icons-absolute-extensionless");
    let icon = dir.write("appimage-icon", "png bytes").expect("write icon");

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
fn resolve_desktop_icon_prefers_scalable_and_high_res_hicolor_icons() {
    let dir = TempDir::new("rayslash-icons-preferred-size");
    dir.write("hicolor/32x32/apps/example.png", "not a real png")
        .expect("write 32px icon");
    dir.write("hicolor/48x48/apps/example.png", "not a real png")
        .expect("write 48px icon");
    let icon_scalable = dir
        .write(
            "hicolor/scalable/apps/example.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .expect("write scalable icon");

    assert_eq!(
        resolve_desktop_icon_in_dirs("example", &[dir.path().to_path_buf()]),
        Some(icon_scalable)
    );

    let bitmap_dir = TempDir::new("rayslash-icons-preferred-bitmap-size");
    bitmap_dir
        .write("hicolor/32x32/apps/bitmap.png", "not a real png")
        .expect("write 32px icon");
    bitmap_dir
        .write("hicolor/48x48/apps/bitmap.png", "not a real png")
        .expect("write 48px icon");
    let icon_128 = bitmap_dir
        .write("hicolor/128x128/apps/bitmap.png", "not a real png")
        .expect("write 128px icon");

    assert_eq!(
        resolve_desktop_icon_in_dirs("bitmap", &[bitmap_dir.path().to_path_buf()]),
        Some(icon_128)
    );

    let hidpi_dir = TempDir::new("rayslash-icons-preferred-hidpi-size");
    hidpi_dir
        .write("hicolor/32x32/apps/hidpi.png", "not a real png")
        .expect("write 32px icon");
    let icon_hidpi = hidpi_dir
        .write("hicolor/256x256@2/apps/hidpi.png", "not a real png")
        .expect("write 256@2 icon");

    assert_eq!(
        resolve_desktop_icon_in_dirs("hidpi", &[hidpi_dir.path().to_path_buf()]),
        Some(icon_hidpi)
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
fn resolve_desktop_icon_falls_back_for_reverse_dns_suffix_icons() {
    let dir = TempDir::new("rayslash-icons-reverse-dns-suffix");
    let theme_dir = dir.create_dir_all("Papirus").expect("create theme dir");
    let icon = dir
        .write(
            "Papirus/48x48/apps/wps-office2019-kprometheus.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .expect("write icon");

    assert_eq!(
        resolve_desktop_icon_in_dirs("com.wps.Office.kprometheus", &[theme_dir]),
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
