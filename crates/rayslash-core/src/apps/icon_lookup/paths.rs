use std::path::{Path, PathBuf};

pub fn resolve_desktop_icon_in_dirs(icon: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let icon = icon.trim();

    if icon.is_empty() {
        return None;
    }

    let icon_path = Path::new(icon);
    if icon_path.is_absolute() {
        if std::env::var_os("FLATPAK_ID").is_some()
            && let Ok(relative_path) = icon_path.strip_prefix("/")
            && let Some(path) =
                supported_existing_absolute_icon(&Path::new("/run/host").join(relative_path))
        {
            return Some(path);
        }
        return supported_existing_absolute_icon(icon_path);
    }

    if icon_path.components().count() > 1 {
        return None;
    }

    for dir in dirs {
        if let Some(path) = resolve_desktop_icon_in_dir(icon, dir) {
            return Some(path);
        }
    }

    None
}

fn resolve_desktop_icon_in_dir(icon: &str, dir: &Path) -> Option<PathBuf> {
    let names = icon_candidate_file_names(icon);

    for name in &names {
        if let Some(path) = supported_existing_icon(&dir.join(name)) {
            return Some(path);
        }
    }

    for theme_root in icon_theme_roots(dir) {
        for relative_dir in icon_theme_app_dirs() {
            let app_dir = theme_root.join(&relative_dir);
            for name in &names {
                if let Some(path) = supported_existing_icon(&app_dir.join(name)) {
                    return Some(path);
                }
            }

            if let Some(path) = supported_existing_icon_with_reverse_dns_suffix(icon, &app_dir) {
                return Some(path);
            }
        }
    }

    None
}

fn icon_theme_roots(dir: &Path) -> Vec<PathBuf> {
    let mut roots = vec![dir.to_path_buf()];

    if dir.file_name().and_then(|name| name.to_str()) != Some("hicolor") {
        roots.push(dir.join("hicolor"));
    }

    roots
}

fn icon_theme_app_dirs() -> Vec<PathBuf> {
    [
        "42x42/apps",
        "48x48/apps",
        "32x32/apps",
        "24x24/apps",
        "22x22/apps",
        "64x64/apps",
        "84x84/apps",
        "96x96/apps",
        "128x128/apps",
        "256x256/apps",
        "512x512/apps",
        "16x16/apps",
        "16x16/symbolic/apps",
        "22x22/symbolic/apps",
        "24x24/symbolic/apps",
        "32x32/symbolic/apps",
        "scalable/apps",
        "apps/scalable",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn icon_candidate_file_names(icon: &str) -> Vec<String> {
    let path = Path::new(icon);

    if path.extension().is_some_and(is_supported_icon_extension) {
        return vec![icon.to_owned()];
    }

    ["svg", "png", "jpg", "jpeg"]
        .into_iter()
        .map(|extension| format!("{icon}.{extension}"))
        .collect()
}

fn supported_existing_icon(path: &Path) -> Option<PathBuf> {
    path.extension()
        .filter(|extension| is_supported_icon_extension(extension))
        .and_then(|_| path.is_file().then(|| path.to_path_buf()))
}

fn supported_existing_absolute_icon(path: &Path) -> Option<PathBuf> {
    if supported_existing_icon(path).is_some() || path.extension().is_none() && path.is_file() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

fn supported_existing_icon_with_reverse_dns_suffix(icon: &str, dir: &Path) -> Option<PathBuf> {
    let suffix = reverse_dns_icon_suffix(icon)?;
    let mut matches = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.to_ascii_lowercase().ends_with(&suffix))
                && path.extension().is_some_and(is_supported_icon_extension)
        })
        .collect::<Vec<_>>();

    matches.sort();
    matches.into_iter().next()
}

fn reverse_dns_icon_suffix(icon: &str) -> Option<String> {
    let path = Path::new(icon);
    let stem = if path.extension().is_some_and(is_supported_icon_extension) {
        path.file_stem()?.to_str()?
    } else {
        icon
    };
    if stem.matches('.').count() < 2 {
        return None;
    }

    let suffix = stem.rsplit('.').next()?;
    if suffix.len() < 10 {
        return None;
    }

    Some(format!("-{}", suffix.to_ascii_lowercase()))
}

fn is_supported_icon_extension(extension: &std::ffi::OsStr) -> bool {
    extension.to_str().is_some_and(|extension| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "svg" | "png" | "jpg" | "jpeg"
        )
    })
}
