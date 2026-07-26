use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rayslash_core::{modules, search};
use slint::{Image, Model, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::ResultItem;

const MAX_MEMORY_ICON_ENTRIES: usize = 256;
const MAX_DISK_ICON_ENTRIES: usize = 64;

struct CachedImage {
    image: Option<Image>,
    last_used: u64,
}

#[derive(Default)]
pub(crate) struct IconImageCache {
    entries: HashMap<PathBuf, CachedImage>,
    clock: u64,
}

impl IconImageCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn get(&mut self, path: &Path) -> Option<&Option<Image>> {
        self.clock = self.clock.wrapping_add(1);
        let entry = self.entries.get_mut(path)?;
        entry.last_used = self.clock;
        Some(&entry.image)
    }

    fn insert(&mut self, path: PathBuf, image: Option<Image>) {
        self.clock = self.clock.wrapping_add(1);
        if self.entries.len() >= MAX_MEMORY_ICON_ENTRIES
            && !self.entries.contains_key(&path)
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(path, _)| path.clone())
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(
            path,
            CachedImage {
                image,
                last_used: self.clock,
            },
        );
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}

pub(crate) fn update_result_items_model(model: &VecModel<ResultItem>, items: Vec<ResultItem>) {
    if model.row_count() == items.len() {
        for (index, item) in items.into_iter().enumerate() {
            model.set_row_data(index, item);
        }
    } else {
        model.set_vec(items);
    }
}

pub(crate) fn to_result_items(
    results: &[search::SearchResult],
    icon_cache: &mut IconImageCache,
) -> Vec<ResultItem> {
    to_result_items_with_images(results, icon_cache, true)
}

pub(crate) fn to_result_items_without_images(
    results: &[search::SearchResult],
    icon_cache: &mut IconImageCache,
) -> Vec<ResultItem> {
    to_result_items_with_images(results, icon_cache, false)
}

fn to_result_items_with_images(
    results: &[search::SearchResult],
    icon_cache: &mut IconImageCache,
    load_images: bool,
) -> Vec<ResultItem> {
    results
        .iter()
        .map(|result| {
            let icon = result_icon(result, icon_cache, load_images);

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

pub(crate) fn load_icon_image(path: &Path, icon_cache: &mut IconImageCache) -> Option<Image> {
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
    icon_cache.insert(path.to_path_buf(), image.clone());
    image
}

fn load_favicon_image(path: &Path, icon_cache: &mut IconImageCache) -> Option<Image> {
    if let Some(cached) = icon_cache.get(path) {
        return cached.clone();
    }

    let image = image::open(path).ok().map(|decoded| {
        let resized = resize_favicon(&decoded.to_rgba8());
        let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(resized.as_raw(), 32, 32);
        Image::from_rgba8(buffer)
    });
    icon_cache.insert(path.to_path_buf(), image.clone());
    image
}

fn resize_favicon(source: &image::RgbaImage) -> image::RgbaImage {
    image::imageops::resize(source, 32, 32, image::imageops::FilterType::Lanczos3)
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
        prune_disk_icon_cache(&cache_dir, &cache_path);
    }

    Some(cache_path)
}

fn prune_disk_icon_cache(cache_dir: &Path, keep: &Path) {
    let mut files = fs::read_dir(cache_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then(|| {
                (
                    entry.path(),
                    metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                )
            })
        })
        .collect::<Vec<_>>();
    if files.len() <= MAX_DISK_ICON_ENTRIES {
        return;
    }
    files.sort_by_key(|(_, modified)| *modified);
    let remove_count = files.len() - MAX_DISK_ICON_ENTRIES;
    for (path, _) in files
        .into_iter()
        .filter(|(path, _)| path != keep)
        .take(remove_count)
    {
        let _ = fs::remove_file(path);
    }
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

fn result_icon(
    result: &search::SearchResult,
    icon_cache: &mut IconImageCache,
    load_images: bool,
) -> RowIcon {
    let module_kind = match &result.kind {
        search::SearchResultKind::Module { module_id, .. } => match module_id.as_str() {
            modules::CALCULATOR_MODULE_ID => "calculator",
            modules::CURRENCY_MODULE_ID => "currency",
            modules::TIME_MODULE_ID => "time",
            modules::TIMERS_MODULE_ID => "timers",
            modules::WEB_SEARCH_MODULE_ID => "web-search",
            _ => "module",
        },
        _ => "module",
    };

    match &result.icon {
        search::SearchResultIcon::Module {
            path: Some(path), ..
        } => {
            if uses_embedded_module_glyph(module_kind) {
                fallback_icon(module_kind, "")
            } else if load_images
                && let Some(image) = if module_kind == "web-search" {
                    load_favicon_image(path, icon_cache)
                } else {
                    load_icon_image(path, icon_cache)
                }
            {
                RowIcon {
                    image,
                    has_image: true,
                    kind: module_kind,
                    text: String::new(),
                }
            } else {
                fallback_icon(module_kind, "")
            }
        }
        search::SearchResultIcon::Module { label, path: None } => {
            fallback_icon_owned(module_kind, label.clone())
        }
        search::SearchResultIcon::App { path: Some(path) } => {
            if load_images && let Some(image) = load_icon_image(path, icon_cache) {
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

fn uses_embedded_module_glyph(kind: &str) -> bool {
    matches!(kind, "calculator" | "currency" | "time" | "timers")
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

    #[test]
    fn memory_icon_cache_evicts_the_least_recently_used_entry() {
        let mut cache = IconImageCache::new();
        for index in 0..=MAX_MEMORY_ICON_ENTRIES {
            cache.insert(PathBuf::from(format!("/icon/{index}")), None);
        }

        assert_eq!(cache.entries.len(), MAX_MEMORY_ICON_ENTRIES);
        assert!(!cache.entries.contains_key(Path::new("/icon/0")));
        assert!(
            cache
                .entries
                .contains_key(Path::new(&format!("/icon/{MAX_MEMORY_ICON_ENTRIES}")))
        );
    }

    #[test]
    fn web_search_module_rows_use_the_favicon_display_kind() {
        let result = search::SearchResult {
            title: "Search YouTube for rust".into(),
            flair: String::new(),
            subtitle: "https://www.youtube.com/results?search_query=rust".into(),
            icon: search::SearchResultIcon::Module {
                label: "youtube".into(),
                path: None,
            },
            kind: search::SearchResultKind::Module {
                module_id: modules::WEB_SEARCH_MODULE_ID.into(),
                result_id: "web-search:youtube:rust".into(),
                action: search::ModuleAction::None,
                score: None,
            },
        };

        let icon = result_icon(&result, &mut IconImageCache::new(), true);
        assert_eq!(icon.kind, "web-search");
    }

    #[test]
    fn favicons_are_prefiltered_to_their_display_size() {
        let source = image::RgbaImage::new(144, 144);
        let resized = resize_favicon(&source);

        assert_eq!(resized.dimensions(), (32, 32));
    }

    #[test]
    fn official_module_rows_use_the_same_glyph_kind_as_settings() {
        for (module_id, expected) in [
            (modules::CALCULATOR_MODULE_ID, "calculator"),
            (modules::CURRENCY_MODULE_ID, "currency"),
            (modules::TIME_MODULE_ID, "time"),
            (modules::TIMERS_MODULE_ID, "timers"),
        ] {
            let result = search::SearchResult {
                title: "result".into(),
                flair: String::new(),
                subtitle: String::new(),
                icon: search::SearchResultIcon::Module {
                    label: String::new(),
                    path: Some(PathBuf::from("/unused/package/icon.svg")),
                },
                kind: search::SearchResultKind::Module {
                    module_id: module_id.into(),
                    result_id: "result".into(),
                    action: search::ModuleAction::None,
                    score: None,
                },
            };

            let icon = result_icon(&result, &mut IconImageCache::new(), true);
            assert_eq!(icon.kind, expected);
            assert!(!icon.has_image);
        }
    }
}
