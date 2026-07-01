use std::{collections::HashMap, path::PathBuf};

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
                subtitle: result.subtitle.clone().into(),
                icon: icon.image,
                has_icon: icon.has_image,
                icon_kind: icon.kind.into(),
                icon_text: icon.text.into(),
            }
        })
        .collect()
}

pub(crate) fn load_icon_image(path: &PathBuf, icon_cache: &mut IconImageCache) -> Option<Image> {
    if let Some(cached) = icon_cache.get(path) {
        return cached.clone();
    }

    let image = Image::load_from_path(path).ok();
    icon_cache.insert(path.clone(), image.clone());
    image
}

struct RowIcon {
    image: Image,
    has_image: bool,
    kind: &'static str,
    text: &'static str,
}

fn result_icon(icon: &search::SearchResultIcon, icon_cache: &mut IconImageCache) -> RowIcon {
    match icon {
        search::SearchResultIcon::App { path: Some(path) } => {
            if let Some(image) = load_icon_image(path, icon_cache) {
                RowIcon {
                    image,
                    has_image: true,
                    kind: "app",
                    text: "",
                }
            } else {
                fallback_icon("app", "")
            }
        }
        search::SearchResultIcon::App { path: None } => fallback_icon("app", ""),
        search::SearchResultIcon::Calculator => fallback_icon("calculator", ""),
        search::SearchResultIcon::ProjectFolder => fallback_icon("folder", ""),
        search::SearchResultIcon::Placeholder => fallback_icon("placeholder", ""),
    }
}

fn fallback_icon(kind: &'static str, text: &'static str) -> RowIcon {
    RowIcon {
        image: Image::default(),
        has_image: false,
        kind,
        text,
    }
}
