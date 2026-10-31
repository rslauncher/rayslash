use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use rayslash_core::{actions, apps};
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
    let mut seen_commands = HashSet::new();
    apps.iter()
        .filter(|app| app.is_folder_opener_candidate())
        .filter_map(|app| {
            let command = picker_command_for_app(app);
            if command.is_empty() || !seen_commands.insert(command.clone()) {
                return None;
            }

            let icon = app
                .icon_path
                .as_ref()
                .and_then(|path| load_icon_image(path, icon_cache));

            Some(AppChoiceItem {
                name: app.name.clone().into(),
                command_label: if is_flatpak_app(app) {
                    "Flatpak".into()
                } else {
                    command.clone().into()
                },
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

    if is_flatpak_app(app) {
        // GLib expands desktop-entry file codes and Flatpak's forwarding
        // markers with the folder in the correct position. Appending a path
        // to a stripped Exec line puts it outside the forwarding markers.
        return command_text(&actions::CommandSpec {
            program: "gio".into(),
            args: vec!["launch".into(), app.desktop_file.as_os_str().to_owned()],
        });
    }

    let command = apps::parse_exec_command(&app.exec).unwrap_or_else(|| app.command.clone());
    command_text(&command)
}

fn is_flatpak_app(app: &apps::DesktopApp) -> bool {
    apps::parse_exec_command(&app.exec).is_some_and(|command| {
        Path::new(&command.program)
            .file_name()
            .is_some_and(|name| name == "flatpak")
            && command
                .args
                .first()
                .is_some_and(|argument| argument == "run")
    })
}

pub(crate) fn resolve_alternate_opener_command(command: &str, apps: &[apps::DesktopApp]) -> String {
    alternate_opener_app(command, apps)
        .filter(|app| is_flatpak_app(app))
        .map(picker_command_for_app)
        .unwrap_or_else(|| command.to_owned())
}

fn command_text(command: &actions::CommandSpec) -> String {
    std::iter::once(&command.program)
        .chain(command.args.iter())
        .map(|part| quote_command_part(&part.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_command_part(part: &str) -> String {
    if !part.is_empty()
        && !part
            .chars()
            .any(|character| character.is_ascii_whitespace() || matches!(character, '"' | '\\'))
    {
        return part.to_owned();
    }

    format!("\"{}\"", part.replace('\\', "\\\\").replace('"', "\\\""))
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

    let parsed = actions::parse_action_command(command);
    apps.iter()
        .find(|app| {
            parsed.is_some()
                && (actions::parse_action_command(&picker_command_for_app(app)) == parsed
                    || apps::parse_exec_command(&app.exec) == parsed
                    || parsed.as_ref() == Some(&app.command))
        })
        .or_else(|| {
            // A wrapper executable alone does not identify the application.
            (!matches!(command_name.as_str(), "flatpak" | "gio"))
                .then(|| {
                    apps.iter().find(|app| {
                        command_basename(&app.command.program.to_string_lossy()) == command_name
                    })
                })
                .flatten()
        })
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
    let program = actions::parse_action_command(command)
        .map(|command| command.program.to_string_lossy().into_owned())
        .unwrap_or_else(|| command.trim().to_owned());
    Path::new(&program)
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
    accent_color_for_icon(command, icon_path.map(PathBuf::as_path))
}

pub(crate) fn accent_color_for_icon(seed: &str, icon_path: Option<&Path>) -> Color {
    icon_path
        .and_then(|path| svg_accent_color(path).or_else(|| raster_accent_color(path)))
        .unwrap_or_else(|| fallback_accent_color(seed))
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

fn raster_accent_color(path: &Path) -> Option<Color> {
    let image = image::open(path).ok()?.thumbnail(32, 32).to_rgb8();
    let mut colors = image
        .pixels()
        .map(|pixel| (pixel[0], pixel[1], pixel[2]))
        .filter_map(|(red, green, blue)| {
            let score = color_score(red, green, blue);
            (score > 0).then_some((score, red, green, blue))
        })
        .collect::<Vec<_>>();
    colors.sort_by_key(|(score, _, _, _)| std::cmp::Reverse(*score));
    let selected = colors.iter().take(24).collect::<Vec<_>>();
    if selected.is_empty() {
        return None;
    }
    let count = selected.len() as u32;
    let red = selected
        .iter()
        .map(|(_, red, _, _)| u32::from(*red))
        .sum::<u32>()
        / count;
    let green = selected
        .iter()
        .map(|(_, _, green, _)| u32::from(*green))
        .sum::<u32>()
        / count;
    let blue = selected
        .iter()
        .map(|(_, _, _, blue)| u32::from(*blue))
        .sum::<u32>()
        / count;
    Some(muted_background_color(red as u8, green as u8, blue as u8))
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

    #[test]
    fn app_choice_items_deduplicate_equivalent_desktop_exec_commands() {
        let mut code = app(
            "Visual Studio Code",
            "/usr/share/code/code",
            Vec::new(),
            vec!["TextEditor", "Development", "IDE"],
        );
        code.id = "code.desktop".into();
        code.exec = "/usr/share/code/code %U".into();
        let mut system_code = code.clone();
        system_code.id = "com.microsoft.VSCode.desktop".into();
        system_code.exec = "\"/usr/share/code/code\"   %F".into();
        let zed = app("Zed", "zed", Vec::new(), vec!["IDE"]);

        let choices =
            to_app_choice_items(&[code, system_code, zed], &mut IconImageCache::default());

        assert_eq!(
            choices
                .iter()
                .map(|choice| (choice.name.as_str(), choice.command.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("Visual Studio Code", "/usr/share/code/code"),
                ("Zed", "zed"),
            ]
        );
    }

    #[test]
    fn app_choice_items_preserve_distinct_arguments_and_executable_paths() {
        let code = app(
            "Visual Studio Code",
            "/usr/share/code/code",
            Vec::new(),
            vec!["IDE"],
        );
        let mut new_window = code.clone();
        new_window.exec = "/usr/share/code/code --new-window %U".into();
        let other_installation = app(
            "Visual Studio Code",
            "/opt/code/code",
            Vec::new(),
            vec!["IDE"],
        );

        let choices = to_app_choice_items(
            &[code, new_window, other_installation],
            &mut IconImageCache::default(),
        );

        assert_eq!(
            choices
                .iter()
                .map(|choice| choice.command.as_str())
                .collect::<Vec<_>>(),
            vec![
                "/usr/share/code/code",
                "/usr/share/code/code --new-window",
                "/opt/code/code",
            ]
        );
    }

    #[test]
    fn app_choice_items_collapse_terminals_using_the_shared_opener() {
        let apps = [
            app("Terminal", "ptyxis", Vec::new(), vec!["TerminalEmulator"]),
            app("Konsole", "konsole", Vec::new(), vec!["TerminalEmulator"]),
        ];

        let choices = to_app_choice_items(&apps, &mut IconImageCache::default());

        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].name.as_str(), "Terminal");
        assert_eq!(choices[0].command.as_str(), "xdg-terminal-exec");
    }

    #[test]
    fn dbus_activatable_picker_choices_use_desktop_exec_instead_of_gio_launcher() {
        let mut files = app("Files", "gio", vec!["inode/directory"], Vec::new());
        files.exec = "nautilus --new-window %U".into();
        files.command.args = vec![
            OsString::from("launch"),
            OsString::from("/usr/share/applications/org.gnome.Nautilus.desktop"),
        ];
        files.dbus_activatable = true;
        let mut analyzer = app(
            "Disk Usage Analyzer",
            "gio",
            vec!["inode/directory"],
            Vec::new(),
        );
        analyzer.exec = "baobab %U".into();
        analyzer.command.args = vec![
            OsString::from("launch"),
            OsString::from("/usr/share/applications/org.gnome.baobab.desktop"),
        ];
        analyzer.dbus_activatable = true;

        let choices = to_app_choice_items(&[files, analyzer], &mut IconImageCache::default());
        assert_eq!(choices[0].command.as_str(), "nautilus --new-window");
        assert_eq!(choices[1].command.as_str(), "baobab");
    }

    #[test]
    fn flatpak_picker_preserves_the_name_and_uses_desktop_file_forwarding() {
        let desktop_file =
            PathBuf::from("/var/lib/flatpak/exports/share/applications/io.bassi.Amberol.desktop");
        let amberol = apps::parse_desktop_entry(
            "[Desktop Entry]\nType=Application\nName=Amberol\nExec=/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=amberol --file-forwarding io.bassi.Amberol @@u %U @@\nMimeType=audio/mpeg;inode/directory;\nCategories=GNOME;GTK;Music;Audio;AudioVideo;\n",
            "io.bassi.Amberol.desktop".into(), desktop_file.clone(),
        ).unwrap();
        let choices = to_app_choice_items(&[amberol], &mut IconImageCache::default());
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].name.as_str(), "Amberol");
        assert_eq!(choices[0].command_label.as_str(), "Flatpak");
        let folder = Path::new("/home/example/Music with spaces/日本語");
        let command = actions::open_project_in_editor_command(folder, &choices[0].command);
        assert_eq!(command.program, "gio");
        assert_eq!(
            command.args,
            vec![
                "launch".into(),
                desktop_file.into_os_string(),
                folder.as_os_str().to_owned()
            ]
        );
    }

    #[test]
    fn flatpak_wrapper_does_not_confuse_different_apps_or_saved_commands() {
        let mut amberol = app(
            "Amberol",
            "/usr/bin/flatpak",
            vec!["inode/directory"],
            Vec::new(),
        );
        amberol.desktop_file =
            "/var/lib/flatpak/exports/share/applications/io.bassi.Amberol.desktop".into();
        amberol.exec = "/usr/bin/flatpak run --file-forwarding io.bassi.Amberol @@u %U @@".into();
        let mut editor = app(
            "Editor",
            "/usr/bin/flatpak",
            vec!["inode/directory"],
            Vec::new(),
        );
        editor.desktop_file =
            "/var/lib/flatpak/exports/share/applications/org.example.Editor.desktop".into();
        editor.exec = "/usr/bin/flatpak run --file-forwarding org.example.Editor @@ %F @@".into();
        let saved = command_text(&apps::parse_exec_command(&editor.exec).unwrap());
        let picker = picker_command_for_app(&editor);
        let apps = [amberol, editor];
        assert_eq!(alternate_opener_app(&saved, &apps).unwrap().name, "Editor");
        assert_eq!(alternate_opener_app(&picker, &apps).unwrap().name, "Editor");
        assert_eq!(resolve_alternate_opener_command(&saved, &apps), picker);
        assert!(alternate_opener_app("flatpak run org.example.Unknown", &apps).is_none());
        assert!(alternate_opener_app("gio launch /missing.desktop", &apps).is_none());
    }

    #[test]
    fn picker_command_text_round_trips_static_arguments() {
        let command = CommandSpec {
            program: OsString::from("/opt/My Editor/bin/editor"),
            args: vec![
                OsString::from("--new-window"),
                OsString::from("a \\\"b\\\""),
            ],
        };
        let text = command_text(&command);

        assert_eq!(actions::parse_action_command(&text), Some(command));
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
