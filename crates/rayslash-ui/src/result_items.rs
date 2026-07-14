use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use rayslash_core::search;
use slint::Image;

use crate::ResultItem;

pub(crate) type IconImageCache = HashMap<PathBuf, Option<Image>>;

pub(crate) fn to_result_items(
    results: &[search::SearchResult],
    icon_cache: &mut IconImageCache,
) -> Vec<ResultItem> {
    results
        .iter()
        .map(|result| {
            let icon = result_icon(&result.icon, icon_cache);

            ResultItem {
                title: result.title.clone().into(),
                flair: result.flair.clone().into(),
                subtitle: result.subtitle.clone().into(),
                subtitle_tooltip: subtitle_tooltip(result).into(),
                icon: icon.image,
                has_icon: icon.has_image,
                icon_kind: icon.kind.into(),
                icon_text: icon.text.into(),
            }
        })
        .collect()
}

fn subtitle_tooltip(result: &search::SearchResult) -> String {
    match &result.kind {
        search::SearchResultKind::Project { path } => path.display().to_string(),
        search::SearchResultKind::App { .. } if result.subtitle != "Application" => {
            result.subtitle.clone()
        }
        search::SearchResultKind::NoResults { query } => {
            format!("No enabled provider matched \"{query}\"")
        }
        search::SearchResultKind::Module { .. } => result.subtitle.clone(),
        _ => String::new(),
    }
}

pub(crate) fn load_icon_image(path: &PathBuf, icon_cache: &mut IconImageCache) -> Option<Image> {
    if let Some(cached) = icon_cache.get(path) {
        return cached.clone();
    }

    // Slint reports decode failures to stderr. Sniff extensionless AppImage-style
    // candidates first so unrelated non-image files fail quietly and use the
    // normal fallback icon, while named image files retain useful diagnostics.
    let image = if path.extension().is_none() {
        load_extensionless_icon_image(path)
    } else {
        Image::load_from_path(path).ok()
    };
    icon_cache.insert(path.clone(), image.clone());
    image
}

fn load_extensionless_icon_image(path: &Path) -> Option<Image> {
    if path.extension().is_some() {
        return None;
    }

    let cache_path = cached_extensionless_icon_path(path)?;
    Image::load_from_path(&cache_path).ok()
}

fn cached_extensionless_icon_path(path: &Path) -> Option<PathBuf> {
    let bytes = fs::read(path).ok()?;
    let extension = image_extension_from_bytes(&bytes)?;
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("rayslash/icons");

    fs::create_dir_all(&cache_dir).ok()?;
    let cache_path = cache_dir.join(format!("{}.{extension}", icon_cache_key(path)));

    if !cache_path.is_file() {
        fs::write(&cache_path, bytes).ok()?;
    }

    Some(cache_path)
}

fn image_extension_from_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }

    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpg");
    }

    let trimmed = bytes
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .collect::<Vec<_>>();
    if trimmed.starts_with(b"<svg") || trimmed.starts_with(b"<?xml") {
        return Some("svg");
    }

    None
}

fn icon_cache_key(path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);

    if let Ok(metadata) = fs::metadata(path) {
        metadata.len().hash(&mut hasher);
        if let Ok(modified) = metadata.modified()
            && let Ok(duration) = modified.duration_since(UNIX_EPOCH)
        {
            duration.as_nanos().hash(&mut hasher);
        }
    }

    hasher.finish()
}

struct RowIcon {
    image: Image,
    has_image: bool,
    kind: &'static str,
    text: String,
}

fn result_icon(icon: &search::SearchResultIcon, icon_cache: &mut IconImageCache) -> RowIcon {
    match icon {
        search::SearchResultIcon::Module {
            path: Some(path), ..
        } => {
            if let Some(image) = load_icon_image(path, icon_cache) {
                RowIcon {
                    image,
                    has_image: true,
                    kind: "module",
                    text: String::new(),
                }
            } else {
                fallback_icon("module", "")
            }
        }
        search::SearchResultIcon::Module { label, path: None } => {
            fallback_icon_owned("module", label.clone())
        }
        search::SearchResultIcon::App { path: Some(path) } => {
            if let Some(image) = load_icon_image(path, icon_cache) {
                RowIcon {
                    image,
                    has_image: true,
                    kind: "app",
                    text: String::new(),
                }
            } else {
                fallback_icon("app", "")
            }
        }
        search::SearchResultIcon::App { path: None } => fallback_icon("app", ""),
        search::SearchResultIcon::ProjectFolder => fallback_icon("folder", ""),
        search::SearchResultIcon::Placeholder => fallback_icon("placeholder", ""),
    }
}

fn fallback_icon(kind: &'static str, text: &'static str) -> RowIcon {
    fallback_icon_owned(kind, text.to_owned())
}

fn fallback_icon_owned(kind: &'static str, text: String) -> RowIcon {
    RowIcon {
        image: Image::default(),
        has_image: false,
        kind,
        text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_extension_from_bytes_detects_supported_extensionless_icons() {
        assert_eq!(
            image_extension_from_bytes(b"\x89PNG\r\n\x1a\nrest"),
            Some("png")
        );
        assert_eq!(
            image_extension_from_bytes(&[0xff, 0xd8, 0xff, 0x00]),
            Some("jpg")
        );
        assert_eq!(
            image_extension_from_bytes(b"  <svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            Some("svg")
        );
        assert_eq!(image_extension_from_bytes(b"not an icon"), None);
    }
}
