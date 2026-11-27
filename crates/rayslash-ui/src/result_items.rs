use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rayslash_core::{modules, search};
use slint::{Image, Model, VecModel};

use crate::{
    ResultItem,
    icon_raster::{
        RESULT_APP_ICON_EDGE, RESULT_FAVICON_EDGE, load_bitmap_at_physical_edge, physical_icon_edge,
    },
};

const MAX_MEMORY_ICON_ENTRIES: usize = 256;
const MAX_MEMORY_ICON_BYTES: usize = 4 * 1024 * 1024;
const MAX_DISK_ICON_ENTRIES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IconCacheKey {
    path: PathBuf,
    physical_edge: u32,
}

struct CachedImage {
    image: Option<Image>,
    last_used: u64,
    stamp: Option<IconStamp>,
    bytes: usize,
}

#[derive(PartialEq, Eq)]
struct IconStamp {
    len: u64,
    modified: SystemTime,
}

fn icon_stamp(path: &Path) -> Option<IconStamp> {
    let metadata = fs::metadata(path).ok()?;
    Some(IconStamp {
        len: metadata.len(),
        modified: metadata.modified().ok()?,
    })
}

#[derive(Default)]
pub(crate) struct IconImageCache {
    entries: HashMap<IconCacheKey, CachedImage>,
    clock: u64,
    bytes: usize,
}

impl IconImageCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn get(&mut self, key: &IconCacheKey) -> Option<&Option<Image>> {
        self.clock = self.clock.wrapping_add(1);
        if self
            .entries
            .get(key)
            .is_some_and(|entry| entry.stamp != icon_stamp(&key.path))
        {
            self.remove(key);
            return None;
        }
        let entry = self.entries.get_mut(key)?;
        entry.last_used = self.clock;
        Some(&entry.image)
    }

    fn insert(&mut self, key: IconCacheKey, image: Option<Image>) {
        self.clock = self.clock.wrapping_add(1);
        self.remove(&key);
        let stamp = icon_stamp(&key.path);
        let bytes = image
            .as_ref()
            .map_or(0, |image| image_weight(image, &key.path, stamp.as_ref()));
        // An oversized image can still be displayed; don't retain another
        // cache reference that exceeds the whole budget.
        if bytes > MAX_MEMORY_ICON_BYTES {
            return;
        }
        while self.entries.len() >= MAX_MEMORY_ICON_ENTRIES
            || self.bytes.saturating_add(bytes) > MAX_MEMORY_ICON_BYTES
        {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.remove(&oldest);
        }
        self.bytes += bytes;
        self.entries.insert(
            key,
            CachedImage {
                image,
                last_used: self.clock,
                stamp,
                bytes,
            },
        );
    }

    pub(crate) fn invalidate_changed(&mut self) {
        self.entries
            .retain(|key, entry| entry.stamp == icon_stamp(&key.path));
        self.bytes = self.entries.values().map(|entry| entry.bytes).sum();
    }

    pub(crate) fn clear_scaled_bitmaps(&mut self) {
        self.entries.retain(|key, _| key.physical_edge == 0);
        self.bytes = self.entries.values().map(|entry| entry.bytes).sum();
    }

    fn remove(&mut self, key: &IconCacheKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.bytes -= entry.bytes;
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

fn is_svg_path(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("svg") || extension.eq_ignore_ascii_case("svgz")
    })
}

fn image_weight(image: &Image, path: &Path, stamp: Option<&IconStamp>) -> usize {
    let image_path = image.path().unwrap_or(path);
    if is_svg_path(image_path) {
        // Slint exposes no parsed-SVG allocation size. Budget a conservative
        // estimate from source size while keeping vectors scalable. This is
        // an estimate, not a bound on the framework's own SVG/image caches.
        let source_bytes = stamp.map_or(0, |stamp| stamp.len);
        return usize::try_from(source_bytes)
            .unwrap_or(usize::MAX)
            .saturating_mul(8)
            .saturating_add(4096);
    }
    let size = image.size();
    (size.width as usize)
        .saturating_mul(size.height as usize)
        .saturating_mul(4)
}

pub(crate) fn update_result_items_model(model: &VecModel<ResultItem>, items: Vec<ResultItem>) {
    let shared_count = model.row_count().min(items.len());
    for (index, item) in items[..shared_count].iter().enumerate() {
        if !model
            .row_data(index)
            .is_some_and(|current| same_result_item(&current, item))
        {
            model.set_row_data(index, item.clone());
        }
    }
    while model.row_count() > items.len() {
        model.remove(model.row_count() - 1);
    }
    if items.len() > shared_count {
        model.extend(items.into_iter().skip(shared_count));
    }
}

pub(crate) fn hydrate_result_icon_rows(
    model: &VecModel<ResultItem>,
    results: &[search::SearchResult],
    scale_factor: f32,
    icon_cache: &mut IconImageCache,
    start: usize,
    count: usize,
) {
    let end = (start + count).min(results.len());
    if start >= end {
        return;
    }
    for (offset, item) in to_result_items(&results[start..end], scale_factor, icon_cache)
        .into_iter()
        .enumerate()
    {
        let index = start + offset;
        if !model
            .row_data(index)
            .is_some_and(|current| same_result_item(&current, &item))
        {
            model.set_row_data(index, item);
        }
    }
}

fn same_result_item(current: &ResultItem, next: &ResultItem) -> bool {
    current.title == next.title
        && current.flair == next.flair
        && current.subtitle == next.subtitle
        && current.subtitle_tooltip == next.subtitle_tooltip
        && current.icon_kind == next.icon_kind
        && current.icon_text == next.icon_text
        && current.has_icon == next.has_icon
        // Slint's empty Image does not compare equal to another empty Image.
        && (!current.has_icon || current.icon == next.icon)
}

// Covers the initial window, including compact rows and a scroll buffer.
pub(crate) const STARTUP_ICON_ROWS: usize = 8;

pub(crate) fn to_initial_result_items(
    results: &[search::SearchResult],
    scale_factor: f32,
    icon_cache: &mut IconImageCache,
) -> Vec<ResultItem> {
    to_result_items_with_image_limit(results, scale_factor, icon_cache, STARTUP_ICON_ROWS)
}

pub(crate) fn to_result_items(
    results: &[search::SearchResult],
    scale_factor: f32,
    icon_cache: &mut IconImageCache,
) -> Vec<ResultItem> {
    to_result_items_with_image_limit(results, scale_factor, icon_cache, usize::MAX)
}

fn to_result_items_with_image_limit(
    results: &[search::SearchResult],
    scale_factor: f32,
    icon_cache: &mut IconImageCache,
    image_limit: usize,
) -> Vec<ResultItem> {
    results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            let icon = result_icon(result, scale_factor, icon_cache, index < image_limit);

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

pub(crate) fn load_icon_image(
    path: &Path,
    logical_edge: u32,
    scale_factor: f32,
    icon_cache: &mut IconImageCache,
) -> Option<Image> {
    let physical_edge = if is_svg_path(path) {
        0
    } else {
        physical_icon_edge(logical_edge, scale_factor)
    };
    let key = IconCacheKey {
        path: path.to_path_buf(),
        physical_edge,
    };
    if let Some(cached) = icon_cache.get(&key) {
        return cached.clone();
    }

    // Slint reports decode failures to stderr. Sniff extensionless AppImage-style
    // candidates first so unrelated non-image files fail quietly and use the
    // normal fallback icon, while named image files retain useful diagnostics.
    let image = if path.extension().is_none() {
        load_extensionless_icon_image(path, physical_edge)
    } else {
        load_sized_icon_image(path, physical_edge)
    };
    icon_cache.insert(key, image.clone());
    image
}

fn load_sized_icon_image(path: &Path, physical_edge: u32) -> Option<Image> {
    if is_svg_path(path) {
        // Keep SVGs scalable so Slint's renderer rasterizes them directly at
        // the exact physical display size.
        return Image::load_from_path(path).ok();
    }
    load_bitmap_at_physical_edge(path, physical_edge)
}

fn load_extensionless_icon_image(path: &Path, physical_edge: u32) -> Option<Image> {
    if path.extension().is_some() {
        return None;
    }

    let cache_path = cached_extensionless_icon_path(path)?;
    load_sized_icon_image(&cache_path, physical_edge)
}

fn cached_extensionless_icon_path(path: &Path) -> Option<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("rayslash/icons");

    let key = icon_cache_key(path);
    for extension in ["png", "jpg", "svg"] {
        let cached = cache_dir.join(format!("{key}.{extension}"));
        if cached.is_file() {
            return Some(cached);
        }
    }
    let bytes = fs::read(path).ok()?;
    let extension = image_extension_from_bytes(&bytes)?;
    fs::create_dir_all(&cache_dir).ok()?;
    let cache_path = cache_dir.join(format!("{key}.{extension}"));

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
    scale_factor: f32,
    icon_cache: &mut IconImageCache,
    load_image: bool,
) -> RowIcon {
    let module_kind = match &result.kind {
        search::SearchResultKind::Module { module_id, .. } => match module_id.as_str() {
            modules::CALCULATOR_MODULE_ID => "calculator",
            modules::TEXT_COUNTER_MODULE_ID => "text-counter",
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
            } else if load_image
                && let Some(image) = if module_kind == "web-search" {
                    load_icon_image(path, RESULT_FAVICON_EDGE, scale_factor, icon_cache)
                } else {
                    load_icon_image(path, RESULT_APP_ICON_EDGE, scale_factor, icon_cache)
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
            if load_image
                && let Some(image) =
                    load_icon_image(path, RESULT_APP_ICON_EDGE, scale_factor, icon_cache)
            {
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
    matches!(
        kind,
        "calculator" | "text-counter" | "currency" | "time" | "timers"
    )
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
    use slint::SharedPixelBuffer;

    fn key(path: impl Into<PathBuf>) -> IconCacheKey {
        IconCacheKey {
            path: path.into(),
            physical_edge: RESULT_APP_ICON_EDGE,
        }
    }

    #[test]
    fn bitmap_icons_are_rescaled_to_exact_physical_display_size() {
        let directory = std::env::temp_dir().join(format!(
            "rayslash-icon-sizing-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&directory).unwrap();
        let large = directory.join("large.png");
        let small = directory.join("small.png");
        let exact = directory.join("exact.png");
        let color = image::Rgba([30, 90, 150, 255]);
        image::RgbaImage::from_pixel(1024, 512, color)
            .save(&large)
            .unwrap();
        image::RgbaImage::from_pixel(24, 16, color)
            .save(&small)
            .unwrap();
        image::RgbaImage::from_pixel(40, 40, color)
            .save(&exact)
            .unwrap();

        let mut cache = IconImageCache::new();
        let icon_1x = load_icon_image(&large, RESULT_APP_ICON_EDGE, 1.0, &mut cache).unwrap();
        assert_eq!((icon_1x.size().width, icon_1x.size().height), (40, 20));
        assert_eq!(&icon_1x.to_rgba8().unwrap().as_bytes()[..4], &color.0);
        assert_eq!(
            load_icon_image(&large, RESULT_APP_ICON_EDGE, 1.0, &mut cache),
            Some(icon_1x)
        );

        let icon_2x = load_icon_image(&large, RESULT_APP_ICON_EDGE, 2.0, &mut cache).unwrap();
        assert_eq!((icon_2x.size().width, icon_2x.size().height), (80, 40));

        let icon_small = load_icon_image(&small, RESULT_APP_ICON_EDGE, 1.0, &mut cache).unwrap();
        assert_eq!(
            (icon_small.size().width, icon_small.size().height),
            (40, 27)
        );

        let icon_exact = load_icon_image(&exact, RESULT_APP_ICON_EDGE, 1.0, &mut cache).unwrap();
        assert_eq!(
            (icon_exact.size().width, icon_exact.size().height),
            (40, 40)
        );
        assert_eq!(&icon_exact.to_rgba8().unwrap().as_bytes()[..4], &color.0);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn clear_scaled_bitmaps_keeps_svg_entries_while_dropping_raster_sizes() {
        let mut cache = IconImageCache::new();
        let svg_key = IconCacheKey {
            path: PathBuf::from("/icons/app.svg"),
            physical_edge: 0,
        };
        let bitmap_key = key("/icons/app.png");
        let image = Image::from_rgba8(SharedPixelBuffer::new(40, 40));
        cache.insert(svg_key.clone(), Some(image.clone()));
        cache.insert(bitmap_key.clone(), Some(image));

        cache.clear_scaled_bitmaps();
        assert!(cache.entries.contains_key(&svg_key));
        assert!(!cache.entries.contains_key(&bitmap_key));
    }

    #[test]
    fn refreshing_results_notifies_only_changed_rows() {
        use slint::private_unstable_api::re_exports::{
            ModelChangeListener, ModelChangeListenerContainer,
        };
        use std::{
            cell::{Cell, RefCell},
            pin::Pin,
        };

        #[derive(Default)]
        struct Changes {
            rows: RefCell<Vec<usize>>,
            resets: Cell<usize>,
            added: Cell<usize>,
            removed: Cell<usize>,
        }
        impl ModelChangeListener for Changes {
            fn row_changed(self: Pin<&Self>, row: usize) {
                self.rows.borrow_mut().push(row);
            }
            fn row_added(self: Pin<&Self>, _: usize, count: usize) {
                assert!(count > 0);
                self.added.set(self.added.get() + count);
            }
            fn row_removed(self: Pin<&Self>, _: usize, count: usize) {
                self.removed.set(self.removed.get() + count);
            }
            fn reset(self: Pin<&Self>) {
                self.resets.set(self.resets.get() + 1);
            }
        }
        let image = Image::from_rgba8(SharedPixelBuffer::new(1, 1));
        let first = ResultItem {
            title: "Alpha".into(),
            icon: image,
            has_icon: true,
            ..Default::default()
        };
        let second = ResultItem {
            title: "Beta".into(),
            ..Default::default()
        };
        let model = VecModel::from(vec![first.clone(), second.clone()]);
        let changes = Box::pin(ModelChangeListenerContainer::<Changes>::default());
        model
            .model_tracker()
            .attach_peer(changes.as_ref().model_peer());

        update_result_items_model(&model, vec![first.clone(), second.clone()]);
        assert!(changes.rows.borrow().is_empty());
        assert_eq!(changes.resets.get(), 0);

        let changed = ResultItem {
            flair: "New".into(),
            ..second
        };
        update_result_items_model(&model, vec![first.clone(), changed.clone()]);
        assert_eq!(*changes.rows.borrow(), vec![1]);
        assert_eq!(model.row_data(0), Some(first.clone()));
        assert_eq!(model.row_data(1).unwrap().flair, "New");
        assert_eq!(changes.resets.get(), 0);

        update_result_items_model(&model, vec![first.clone(), changed, ResultItem::default()]);
        assert_eq!(changes.added.get(), 1);
        assert_eq!(*changes.rows.borrow(), vec![1]);
        update_result_items_model(&model, vec![first]);
        assert_eq!(changes.removed.get(), 2);
        assert_eq!(*changes.rows.borrow(), vec![1]);
        update_result_items_model(&model, vec![]);
        assert_eq!(model.row_count(), 0);
        assert_eq!(changes.removed.get(), 3);
        assert_eq!(changes.resets.get(), 0);
    }

    #[test]
    fn icon_cache_retains_unchanged_images_and_retries_changed_or_missing_files() {
        let directory = std::env::temp_dir().join(format!(
            "rayslash-icon-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let unchanged = directory.join("unchanged.png");
        let changed = directory.join("changed.png");
        let missing = directory.join("missing.png");
        fs::write(&unchanged, b"unchanged").unwrap();
        fs::write(&changed, b"old").unwrap();
        let image = Image::from_rgba8(SharedPixelBuffer::new(1, 1));
        let mut cache = IconImageCache::new();
        cache.insert(key(&unchanged), Some(image.clone()));
        cache.insert(key(&changed), None);
        cache.insert(key(&missing), None);
        assert_eq!(cache.get(&key(&unchanged)), Some(&Some(image.clone())));
        assert_eq!(cache.get(&key(&changed)), Some(&None));
        assert_eq!(cache.get(&key(&missing)), Some(&None));

        fs::write(&changed, b"updated image").unwrap();
        fs::write(&missing, b"new image").unwrap();
        assert!(cache.get(&key(&changed)).is_none());
        assert!(cache.get(&key(&missing)).is_none());
        cache.invalidate_changed();
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.get(&key(&unchanged)), Some(&Some(image)));

        fs::remove_file(&unchanged).unwrap();
        cache.invalidate_changed();
        assert!(cache.entries.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn initial_app_rows_include_the_cached_icon() {
        let path = PathBuf::from("/test/app.png");
        let image = Image::from_rgba8(SharedPixelBuffer::new(1, 1));
        let mut cache = IconImageCache::new();
        cache.insert(key(&path), Some(image.clone()));
        let result = search::SearchResult {
            title: "App".into(),
            flair: String::new(),
            subtitle: "Application".into(),
            icon: search::SearchResultIcon::App { path: Some(path) },
            kind: search::SearchResultKind::Placeholder,
        };
        let results = vec![result; STARTUP_ICON_ROWS + 1];
        let rows = to_initial_result_items(&results, 1.0, &mut cache);
        assert!(rows[0].has_icon);
        assert_eq!(rows[0].icon, image.clone());
        assert!(rows[STARTUP_ICON_ROWS - 1].has_icon);
        assert!(!rows[STARTUP_ICON_ROWS].has_icon);
        let model = VecModel::from(rows);
        hydrate_result_icon_rows(&model, &results, 1.0, &mut cache, STARTUP_ICON_ROWS, 4);
        assert!(model.row_data(STARTUP_ICON_ROWS).unwrap().has_icon);
        assert_eq!(model.row_data(0).unwrap().icon, image);
        // A query can shrink the list before the next hydration batch.
        hydrate_result_icon_rows(&model, &[], 1.0, &mut cache, STARTUP_ICON_ROWS, 4);
        assert_eq!(model.row_count(), results.len());
    }

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
            cache.insert(key(format!("/icon/{index}")), None);
        }

        assert_eq!(cache.entries.len(), MAX_MEMORY_ICON_ENTRIES);
        assert!(!cache.entries.contains_key(&key("/icon/0")));
        assert!(
            cache
                .entries
                .contains_key(&key(format!("/icon/{MAX_MEMORY_ICON_ENTRIES}")))
        );
    }

    #[test]
    fn bitmap_byte_budget_evicts_old_images_while_preserving_display_references() {
        let mut cache = IconImageCache::new();
        let image = Image::from_rgba8(SharedPixelBuffer::new(128, 128));
        let capacity = MAX_MEMORY_ICON_BYTES / (128 * 128 * 4);
        for index in 0..capacity {
            cache.insert(key(format!("/icon/{index}")), Some(image.clone()));
        }
        assert!(cache.get(&key("/icon/0")).is_some());
        cache.insert(key("/icon/new"), Some(image.clone()));
        assert!(cache.get(&key("/icon/0")).is_some());
        assert!(cache.get(&key("/icon/1")).is_none());
        assert_eq!(cache.entries.len(), capacity);
        assert_eq!(cache.bytes, MAX_MEMORY_ICON_BYTES);
        cache.clear();
        assert_eq!(cache.bytes, 0);
        assert!(cache.entries.is_empty());
        assert_eq!(image.size().width, 128);
    }

    #[test]
    fn replacement_and_oversized_images_do_not_inflate_cache_accounting() {
        let mut cache = IconImageCache::new();
        let cache_key = key("/icon/replaced");
        cache.insert(
            cache_key.clone(),
            Some(Image::from_rgba8(SharedPixelBuffer::new(128, 128))),
        );
        cache.insert(
            cache_key.clone(),
            Some(Image::from_rgba8(SharedPixelBuffer::new(1, 1))),
        );
        assert_eq!(cache.bytes, 4);
        cache.insert(
            cache_key.clone(),
            Some(Image::from_rgba8(SharedPixelBuffer::new(1025, 1025))),
        );
        assert_eq!(cache.bytes, 0);
        assert!(cache.get(&cache_key).is_none());
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

        let icon = result_icon(&result, 1.0, &mut IconImageCache::new(), true);
        assert_eq!(icon.kind, "web-search");
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

            let icon = result_icon(&result, 1.0, &mut IconImageCache::new(), true);
            assert_eq!(icon.kind, expected);
            assert!(!icon.has_image);
        }
    }
}
