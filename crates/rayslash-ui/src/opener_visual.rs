use std::path::{Path, PathBuf};

use rayslash_core::apps;
use slint::Color;

use crate::result_items::{IconImageCache, load_icon_image};
use crate::{AppChoiceItem, AppWindow};

pub(crate) fn app_icon_count(apps: &[apps::DesktopApp]) -> usize {
    apps.iter().filter(|app| app.icon_path.is_some()).count()
}

pub(crate) fn to_app_choice_items(
    apps: &[apps::DesktopApp],
    icon_cache: &mut IconImageCache,
) -> Vec<AppChoiceItem> {
    apps.iter()
        .filter(|app| app.is_folder_opener_candidate())
        .filter_map(|app| {
            let command = picker_command_for_app(app);
            if command.is_empty() {
                return None;
            }

            let icon = app
                .icon_path
                .as_ref()
                .and_then(|path| load_icon_image(path, icon_cache));

            Some(AppChoiceItem {
                name: app.name.clone().into(),
                command: command.into(),
                icon: icon.clone().unwrap_or_default(),
                has_icon: icon.is_some(),
            })
        })
        .collect()
}

fn picker_command_for_app(app: &apps::DesktopApp) -> String {
    if app.is_terminal_emulator() {
        return "xdg-terminal-exec".to_owned();
    }

    app.command.program.to_string_lossy().trim().to_owned()
}

pub(crate) fn set_alternate_opener_visual(
    ui: &AppWindow,
    command: &str,
    apps: &[apps::DesktopApp],
    icon_cache: &mut IconImageCache,
) {
    let app = alternate_opener_app(command, apps);
    let icon_path = app.and_then(|app| app.icon_path.as_ref());
    let icon = icon_path.and_then(|path| load_icon_image(path, icon_cache));

    ui.set_alternate_folder_opener_icon(icon.clone().unwrap_or_default());
    ui.set_alternate_folder_opener_has_icon(icon.is_some());
    ui.set_alternate_folder_opener_label(opener_label(command).into());
    ui.set_alternate_folder_opener_background(accent_color_for_opener(command, icon_path));
}

fn alternate_opener_app<'a>(
    command: &str,
    apps: &'a [apps::DesktopApp],
) -> Option<&'a apps::DesktopApp> {
    let command_name = command_basename(command);
    if command_name.is_empty() {
        return None;
    }

    apps.iter()
        .find(|app| command_basename(&app.command.program.to_string_lossy()) == command_name)
        .or_else(|| {
            (command_name == "xdg-terminal-exec")
                .then(|| terminal_like_app(apps))
                .flatten()
        })
}

fn terminal_like_app(apps: &[apps::DesktopApp]) -> Option<&apps::DesktopApp> {
    apps.iter().find(|app| {
        let text = format!(
            "{} {} {}",
            app.name,
            app.generic_name.as_deref().unwrap_or_default(),
            app.comment.as_deref().unwrap_or_default()
        )
        .to_ascii_lowercase();
        text.contains("terminal")
    })
}

fn command_basename(command: &str) -> String {
    Path::new(command.trim())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command.trim())
        .to_ascii_lowercase()
}

fn opener_label(command: &str) -> String {
    let command_name = command_basename(command);
    if command_name == "xdg-terminal-exec" || command_name.contains("terminal") {
        return "TM".to_owned();
    }

    let mut label = command_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(2)
        .collect::<String>()
        .to_uppercase();

    if label.is_empty() {
        label = "OP".to_owned();
    }

    label
}

fn accent_color_for_opener(command: &str, icon_path: Option<&PathBuf>) -> Color {
    icon_path
        .and_then(|path| svg_accent_color(path))
        .unwrap_or_else(|| fallback_accent_color(command))
}

fn svg_accent_color(path: &Path) -> Option<Color> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        return None;
    }

    let contents = std::fs::read_to_string(path).ok()?;
    let mut best = None;
    let mut best_score = 0u16;
    let bytes = contents.as_bytes();

    for index in 0..bytes.len().saturating_sub(6) {
        if bytes[index] != b'#' {
            continue;
        }

        let hex = &contents[index + 1..index + 7];
        if !hex.chars().all(|character| character.is_ascii_hexdigit()) {
            continue;
        }

        let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let score = color_score(red, green, blue);

        if score > best_score {
            best_score = score;
            best = Some((red, green, blue));
        }
    }

    best.map(|(red, green, blue)| muted_background_color(red, green, blue))
}

fn color_score(red: u8, green: u8, blue: u8) -> u16 {
    let max = red.max(green).max(blue) as u16;
    let min = red.min(green).min(blue) as u16;
    let saturation = max.saturating_sub(min);
    let brightness = (red as u16 + green as u16 + blue as u16) / 3;

    if !(48..=220).contains(&brightness) || saturation < 24 {
        return 0;
    }

    saturation + brightness / 4
}

fn muted_background_color(red: u8, green: u8, blue: u8) -> Color {
    Color::from_rgb_u8(
        ((red as u16 * 3) / 5).max(24) as u8,
        ((green as u16 * 3) / 5).max(24) as u8,
        ((blue as u16 * 3) / 5).max(24) as u8,
    )
}

fn fallback_accent_color(seed: &str) -> Color {
    let mut hash = 0u32;
    for byte in seed.bytes() {
        hash = hash.wrapping_mul(16777619) ^ u32::from(byte);
    }

    let red = 64 + (hash & 0x3f) as u8;
    let green = 64 + ((hash >> 8) & 0x3f) as u8;
    let blue = 64 + ((hash >> 16) & 0x3f) as u8;

    Color::from_rgb_u8(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayslash_core::actions::CommandSpec;
    use std::ffi::OsString;

    #[test]
    fn opener_label_uses_terminal_and_command_fallbacks() {
        assert_eq!(opener_label("xdg-terminal-exec"), "TM");
        assert_eq!(opener_label("codium"), "CO");
        assert_eq!(opener_label("--"), "OP");
    }

    #[test]
    fn app_choice_items_include_only_directory_openers() {
        let apps = vec![
            app("Files", "nautilus", vec!["inode/directory"], Vec::new()),
            app(
                "Zed",
                "zed",
                Vec::new(),
                vec!["Utility", "TextEditor", "Development", "IDE"],
            ),
            app(
                "Terminal",
                "ptyxis",
                Vec::new(),
                vec!["GNOME", "System", "TerminalEmulator"],
            ),
            app(
                "Calculator",
                "gnome-calculator",
                Vec::new(),
                vec!["Utility", "Calculator"],
            ),
        ];
        let mut icon_cache = IconImageCache::default();

        let choices = to_app_choice_items(&apps, &mut icon_cache);

        assert_eq!(
            choices
                .iter()
                .map(|choice| (choice.name.to_string(), choice.command.to_string()))
                .collect::<Vec<_>>(),
            vec![
                ("Files".to_owned(), "nautilus".to_owned()),
                ("Zed".to_owned(), "zed".to_owned()),
                ("Terminal".to_owned(), "xdg-terminal-exec".to_owned()),
            ]
        );
    }

    fn app(
        name: &str,
        program: &str,
        mime_types: Vec<&str>,
        categories: Vec<&str>,
    ) -> apps::DesktopApp {
        apps::DesktopApp {
            id: format!("{}.desktop", name.to_ascii_lowercase()),
            name: name.to_owned(),
            localized_names: Vec::new(),
            generic_name: None,
            comment: None,
            exec: program.to_owned(),
            icon: None,
            mime_types: mime_types.into_iter().map(str::to_owned).collect(),
            categories: categories.into_iter().map(str::to_owned).collect(),
            keywords: Vec::new(),
            actions: Vec::new(),
            dbus_activatable: false,
            startup_wm_class: None,
            icon_path: None,
            command: CommandSpec {
                program: OsString::from(program),
                args: Vec::new(),
            },
            desktop_file: PathBuf::from(format!("/tmp/{program}.desktop")),
        }
    }
}
