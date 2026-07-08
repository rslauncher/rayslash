use std::time::Duration;

use crate::actions::CommandSpec;

const DEFAULT_DELAY: Duration = Duration::from_secs(30);
const DEFAULT_TIMER_MESSAGE: &str = "Timer finished.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UtilityAction {
    System(SystemAction),
    Timer(TimerAction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemAction {
    pub kind: SystemActionKind,
    pub delay: Duration,
    pub expression: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemActionKind {
    Reboot,
    Shutdown,
    Logout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerAction {
    pub delay: Duration,
    pub message: String,
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtilityActionError {
    pub expression: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DurationMatch {
    start: usize,
    end: usize,
    duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotedSegment {
    start: usize,
    end: usize,
    value: String,
}

pub fn parse_query(query: &str) -> Option<Result<UtilityAction, UtilityActionError>> {
    let expression = query.trim();
    if expression.is_empty() {
        return None;
    }

    if let Some((kind, rest)) = strip_system_action_prefix(expression) {
        return Some(parse_system_action(kind, expression, rest));
    }

    if let Some(rest) = strip_word_prefix(expression, "timer") {
        return Some(parse_timer(expression, rest));
    }

    if let Some(rest) = strip_word_prefix(expression, "remind me")
        .or_else(|| strip_word_prefix(expression, "remind"))
    {
        return Some(parse_reminder(expression, rest));
    }

    None
}

pub fn system_action_command(kind: SystemActionKind) -> CommandSpec {
    match kind {
        SystemActionKind::Reboot => CommandSpec {
            program: "systemctl".into(),
            args: vec!["reboot".into()],
        },
        SystemActionKind::Shutdown => CommandSpec {
            program: "systemctl".into(),
            args: vec!["poweroff".into()],
        },
        SystemActionKind::Logout => logout_command(
            std::env::var("XDG_SESSION_ID").ok(),
            std::env::var("USER").ok(),
        ),
    }
}

pub fn timer_notification_command(message: &str) -> CommandSpec {
    CommandSpec {
        program: "notify-send".into(),
        args: vec![
            "-i".into(),
            "stopwatch".into(),
            "rayslash timer".into(),
            message.into(),
        ],
    }
}

pub fn action_delay(action: &UtilityAction) -> Duration {
    match action {
        UtilityAction::System(action) => action.delay,
        UtilityAction::Timer(action) => action.delay,
    }
}

pub fn action_title(action: &UtilityAction) -> String {
    match action {
        UtilityAction::System(action) => {
            let verb = match action.kind {
                SystemActionKind::Reboot => "Reboot",
                SystemActionKind::Shutdown => "Shut down",
                SystemActionKind::Logout => "Log out",
            };
            if action.delay.is_zero() {
                format!("{verb} now")
            } else {
                format!("{verb} in {}", format_duration(action.delay))
            }
        }
        UtilityAction::Timer(action) => {
            if action.delay.is_zero() {
                format!("Remind now: {}", action.message)
            } else {
                format!(
                    "Remind in {}: {}",
                    format_duration(action.delay),
                    action.message
                )
            }
        }
    }
}

pub fn action_subtitle(action: &UtilityAction) -> String {
    match action {
        UtilityAction::System(action) => match action.kind {
            SystemActionKind::Reboot => "System reboot".to_owned(),
            SystemActionKind::Shutdown => "System shutdown".to_owned(),
            SystemActionKind::Logout => "Log out of the current session".to_owned(),
        },
        UtilityAction::Timer(action) => format!("Notification: {}", action.message),
    }
}

pub fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds == 0 {
        return "now".to_owned();
    }
    if seconds.is_multiple_of(3600) {
        let hours = seconds / 3600;
        return plural(hours, "hour");
    }
    if seconds.is_multiple_of(60) {
        let minutes = seconds / 60;
        return plural(minutes, "minute");
    }
    plural(seconds, "second")
}

fn parse_system_action(
    kind: SystemActionKind,
    expression: &str,
    rest: &str,
) -> Result<UtilityAction, UtilityActionError> {
    let rest = rest.trim();
    let delay = if rest.is_empty() {
        DEFAULT_DELAY
    } else if rest.eq_ignore_ascii_case("now") {
        Duration::ZERO
    } else {
        let segments = quoted_segments(rest).map_err(|message| error(expression, message))?;
        let matches = duration_matches(rest, &quote_spans(&segments));
        match matches.as_slice() {
            [] => return Err(error(expression, "Use now or in <time>.")),
            [duration_match] => {
                let leftover = clean_connector_text(&remove_ranges(
                    rest,
                    &[duration_match_range(duration_match)],
                ));
                if !leftover.is_empty() {
                    return Err(error(expression, "Use now or in <time>."));
                }
                duration_match.duration
            }
            _ => return Err(error(expression, multiple_times_message())),
        }
    };

    Ok(UtilityAction::System(SystemAction {
        kind,
        delay,
        expression: expression.to_owned(),
    }))
}

fn parse_timer(expression: &str, rest: &str) -> Result<UtilityAction, UtilityActionError> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Err(error(expression, "Add a time or message for the timer."));
    }

    let segments = quoted_segments(rest).map_err(|message| error(expression, message))?;
    let spans = quote_spans(&segments);
    let matches = duration_matches(rest, &spans);
    if matches.len() > 1 {
        return Err(error(expression, multiple_times_message()));
    }

    let delay = matches
        .first()
        .map(|duration_match| duration_match.duration)
        .unwrap_or(DEFAULT_DELAY);
    let quoted_message = segments
        .iter()
        .map(|segment| segment.value.trim())
        .find(|value| !value.is_empty())
        .map(str::to_owned);

    let message = quoted_message.unwrap_or_else(|| {
        let mut ranges = spans;
        ranges.extend(matches.iter().map(duration_match_range));
        clean_timer_message(&remove_ranges(rest, &ranges))
    });

    let message = if message.is_empty() {
        if matches.is_empty() {
            return Err(error(expression, "Add a time or message for the timer."));
        }
        DEFAULT_TIMER_MESSAGE.to_owned()
    } else {
        message
    };

    Ok(UtilityAction::Timer(TimerAction {
        delay,
        message,
        expression: expression.to_owned(),
    }))
}

fn parse_reminder(expression: &str, rest: &str) -> Result<UtilityAction, UtilityActionError> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Err(error(expression, reminder_syntax_message()));
    }

    let to_ranges = word_ranges(rest, "to");
    let in_ranges = word_ranges(rest, "in");
    if to_ranges.is_empty() || in_ranges.is_empty() {
        return Err(error(expression, reminder_syntax_message()));
    }

    let parts = reminder_parts(rest, &to_ranges, &in_ranges)
        .ok_or_else(|| error(expression, reminder_syntax_message()))?;
    let segments = quoted_segments(parts.message).map_err(|message| error(expression, message))?;
    let message = segments
        .iter()
        .map(|segment| segment.value.trim())
        .find(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| parts.message.trim().to_owned());
    if message.is_empty() {
        return Err(error(expression, reminder_syntax_message()));
    }

    let time_segments =
        quoted_segments(parts.time).map_err(|message| error(expression, message))?;
    let matches = duration_matches(parts.time, &quote_spans(&time_segments));
    let delay = match matches.as_slice() {
        [duration_match] => duration_match.duration,
        [] => return Err(error(expression, "Add one reminder time.")),
        _ => return Err(error(expression, multiple_times_message())),
    };

    Ok(UtilityAction::Timer(TimerAction {
        delay,
        message,
        expression: expression.to_owned(),
    }))
}

struct ReminderParts<'a> {
    time: &'a str,
    message: &'a str,
}

fn reminder_parts<'a>(
    rest: &'a str,
    to_ranges: &[std::ops::Range<usize>],
    in_ranges: &[std::ops::Range<usize>],
) -> Option<ReminderParts<'a>> {
    if let Some(to_range) = to_ranges.first()
        && let Some(in_range) = in_ranges
            .iter()
            .rev()
            .find(|in_range| in_range.start > to_range.end)
    {
        return Some(ReminderParts {
            message: rest[to_range.end..in_range.start].trim(),
            time: rest[in_range.end..].trim(),
        });
    }

    if let Some(in_range) = in_ranges.first()
        && let Some(to_range) = to_ranges
            .iter()
            .find(|to_range| to_range.start > in_range.end)
    {
        return Some(ReminderParts {
            time: rest[in_range.end..to_range.start].trim(),
            message: rest[to_range.end..].trim(),
        });
    }

    None
}

fn strip_system_action_prefix(query: &str) -> Option<(SystemActionKind, &str)> {
    [
        ("reboot", SystemActionKind::Reboot),
        ("restart", SystemActionKind::Reboot),
        ("shutdown", SystemActionKind::Shutdown),
        ("shut down", SystemActionKind::Shutdown),
        ("poweroff", SystemActionKind::Shutdown),
        ("power off", SystemActionKind::Shutdown),
        ("logout", SystemActionKind::Logout),
        ("log out", SystemActionKind::Logout),
    ]
    .into_iter()
    .find_map(|(prefix, kind)| strip_word_prefix(query, prefix).map(|rest| (kind, rest)))
}

fn strip_word_prefix<'a>(query: &'a str, prefix: &str) -> Option<&'a str> {
    let query = query.trim();
    if query.len() < prefix.len() {
        return None;
    }

    let (head, rest) = query.split_at(prefix.len());
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    if !rest.is_empty() && !rest.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }

    Some(rest.trim())
}

fn quoted_segments(text: &str) -> Result<Vec<QuotedSegment>, &'static str> {
    let mut segments = Vec::new();
    let mut chars = text.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        if ch != '"' && ch != '\'' {
            continue;
        }

        let quote = ch;
        let mut value = String::new();
        let mut end = None;

        while let Some((index, ch)) = chars.next() {
            if ch == '\\' {
                if let Some((_next_index, next)) = chars.next() {
                    value.push(next);
                } else {
                    value.push(ch);
                }
                continue;
            }
            if ch == quote {
                end = Some(index + ch.len_utf8());
                break;
            }
            value.push(ch);
        }

        let Some(end) = end else {
            return Err("Close the quoted message.");
        };

        segments.push(QuotedSegment { start, end, value });
    }

    Ok(segments)
}

fn quote_spans(segments: &[QuotedSegment]) -> Vec<std::ops::Range<usize>> {
    segments
        .iter()
        .map(|segment| segment.start..segment.end)
        .collect()
}

fn duration_matches(text: &str, protected_spans: &[std::ops::Range<usize>]) -> Vec<DurationMatch> {
    let mut matches = Vec::new();
    let mut index = 0;

    while index < text.len() {
        if let Some(span) = protected_spans
            .iter()
            .find(|span| index >= span.start && index < span.end)
        {
            index = span.end;
            continue;
        }

        let Some(ch) = text[index..].chars().next() else {
            break;
        };
        if !ch.is_ascii_digit() {
            index += ch.len_utf8();
            continue;
        }

        let start = index;
        let mut end = index;
        let mut seen_dot = false;
        let mut seen_digit = false;
        for (offset, ch) in text[start..].char_indices() {
            if ch.is_ascii_digit() {
                seen_digit = true;
                end = start + offset + ch.len_utf8();
            } else if ch == '.' && !seen_dot {
                seen_dot = true;
                end = start + offset + ch.len_utf8();
            } else {
                break;
            }
        }

        if !seen_digit {
            index += ch.len_utf8();
            continue;
        }

        let number = match text[start..end].parse::<f64>() {
            Ok(number) if number.is_finite() && number >= 0.0 => number,
            _ => {
                index = end;
                continue;
            }
        };

        let Some((unit, match_end)) = duration_unit_after_number(text, end) else {
            index = end;
            continue;
        };
        let Some(duration) = duration_from_number(number, unit) else {
            index = end;
            continue;
        };

        matches.push(DurationMatch {
            start,
            end: match_end,
            duration,
        });
        index = match_end;
    }

    matches
}

fn duration_unit_after_number(text: &str, number_end: usize) -> Option<(Option<&str>, usize)> {
    let attached_start = number_end;
    let attached_end = word_end(text, attached_start);
    if attached_end > attached_start {
        let unit = &text[attached_start..attached_end];
        return if is_duration_unit(unit) {
            Some((Some(unit), attached_end))
        } else {
            None
        };
    }

    let spaced_start = skip_ascii_whitespace(text, number_end);
    let spaced_end = word_end(text, spaced_start);
    if spaced_end > spaced_start {
        let unit = &text[spaced_start..spaced_end];
        if is_duration_unit(unit) {
            return Some((Some(unit), spaced_end));
        }
    }

    Some((None, number_end))
}

fn word_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in text[start..].char_indices() {
        if !ch.is_ascii_alphabetic() {
            break;
        }
        end = start + offset + ch.len_utf8();
    }
    end
}

fn skip_ascii_whitespace(text: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in text[start..].char_indices() {
        if !ch.is_ascii_whitespace() {
            break;
        }
        end = start + offset + ch.len_utf8();
    }
    end
}

fn is_duration_unit(unit: &str) -> bool {
    duration_unit_multiplier(unit).is_some()
}

fn duration_unit_multiplier(unit: &str) -> Option<f64> {
    match unit.to_ascii_lowercase().as_str() {
        "s" | "sec" | "secs" | "second" | "seconds" => Some(1.0),
        "m" | "min" | "mins" | "minute" | "minutes" => Some(60.0),
        "h" | "hr" | "hrs" | "hour" | "hours" => Some(3600.0),
        _ => None,
    }
}

fn duration_from_number(number: f64, unit: Option<&str>) -> Option<Duration> {
    let multiplier = unit.and_then(duration_unit_multiplier).unwrap_or(1.0);
    let seconds = (number * multiplier).round();
    if seconds < 0.0 || seconds > u64::MAX as f64 {
        return None;
    }
    Some(Duration::from_secs(seconds as u64))
}

fn remove_ranges(text: &str, ranges: &[std::ops::Range<usize>]) -> String {
    let mut output = String::new();
    let mut sorted = ranges.to_vec();
    sorted.sort_by_key(|range| range.start);
    let mut index = 0;

    for range in sorted {
        if range.start > index {
            output.push_str(&text[index..range.start]);
        }
        index = index.max(range.end);
    }
    if index < text.len() {
        output.push_str(&text[index..]);
    }

    output
}

fn duration_match_range(duration_match: &DurationMatch) -> std::ops::Range<usize> {
    duration_match.start..duration_match.end
}

fn clean_timer_message(text: &str) -> String {
    clean_connector_text(text)
}

fn clean_connector_text(text: &str) -> String {
    let mut words = text.split_whitespace().collect::<Vec<_>>();
    while words
        .first()
        .is_some_and(|word| matches_ignore_case(word, &["in", "for", "to"]))
    {
        words.remove(0);
    }
    while words
        .last()
        .is_some_and(|word| matches_ignore_case(word, &["in", "for", "to"]))
    {
        words.pop();
    }
    words.join(" ")
}

fn word_ranges(text: &str, word: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let lower = text.to_ascii_lowercase();
    let mut search_start = 0;

    while let Some(offset) = lower[search_start..].find(word) {
        let start = search_start + offset;
        let end = start + word.len();
        let before_ok = start == 0
            || text[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        let after_ok =
            end == text.len() || text[end..].chars().next().is_some_and(char::is_whitespace);
        if before_ok && after_ok {
            ranges.push(start..end);
        }
        search_start = end;
    }

    ranges
}

fn matches_ignore_case(word: &str, options: &[&str]) -> bool {
    options
        .iter()
        .any(|option| word.eq_ignore_ascii_case(option))
}

fn logout_command(session_id: Option<String>, user: Option<String>) -> CommandSpec {
    if let Some(session_id) = session_id.filter(|session_id| !session_id.trim().is_empty()) {
        return CommandSpec {
            program: "loginctl".into(),
            args: vec!["terminate-session".into(), session_id.into()],
        };
    }

    CommandSpec {
        program: "loginctl".into(),
        args: vec![
            "terminate-user".into(),
            user.filter(|user| !user.trim().is_empty())
                .unwrap_or_else(|| std::env::var("LOGNAME").unwrap_or_default())
                .into(),
        ],
    }
}

fn plural(value: u64, unit: &str) -> String {
    if value == 1 {
        format!("1 {unit}")
    } else {
        format!("{value} {unit}s")
    }
}

fn error(expression: &str, message: &str) -> UtilityActionError {
    UtilityActionError {
        expression: expression.to_owned(),
        message: message.to_owned(),
    }
}

fn multiple_times_message() -> &'static str {
    "More than one time found. Quote the message if needed."
}

fn reminder_syntax_message() -> &'static str {
    "Use: remind me to <message> in <time>."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_system_actions_with_default_delay_now_and_units() {
        let action = parse_query("reboot").expect("query").expect("action");
        assert_eq!(
            action,
            UtilityAction::System(SystemAction {
                kind: SystemActionKind::Reboot,
                delay: Duration::from_secs(30),
                expression: "reboot".to_owned(),
            })
        );

        let action = parse_query("shutdown in 10min")
            .expect("query")
            .expect("action");
        assert_eq!(action_delay(&action), Duration::from_secs(600));

        let action = parse_query("logout now").expect("query").expect("action");
        assert_eq!(action_delay(&action), Duration::ZERO);
    }

    #[test]
    fn parses_timer_messages_and_times_in_loose_order() {
        let action = parse_query("timer 10 feed the cat")
            .expect("query")
            .expect("action");
        assert_eq!(action_delay(&action), Duration::from_secs(10));
        assert!(action_title(&action).contains("feed the cat"));

        let action = parse_query("timer feed the cat 10min")
            .expect("query")
            .expect("action");
        assert_eq!(action_delay(&action), Duration::from_secs(600));

        let action = parse_query("timer for feed the cat")
            .expect("query")
            .expect("action");
        assert_eq!(action_delay(&action), Duration::from_secs(30));
        assert!(action_title(&action).contains("feed the cat"));

        let action = parse_query("timer in 10").expect("query").expect("action");
        assert_eq!(action_delay(&action), Duration::from_secs(10));
        assert!(action_title(&action).contains(DEFAULT_TIMER_MESSAGE));
    }

    #[test]
    fn quoted_timer_message_does_not_create_extra_times() {
        let action = parse_query("timer 'feed 2 cats' 10min")
            .expect("query")
            .expect("action");

        assert_eq!(action_delay(&action), Duration::from_secs(600));
        assert_eq!(
            action,
            UtilityAction::Timer(TimerAction {
                delay: Duration::from_secs(600),
                message: "feed 2 cats".to_owned(),
                expression: "timer 'feed 2 cats' 10min".to_owned(),
            })
        );
    }

    #[test]
    fn timer_reports_multiple_times() {
        let error = parse_query("timer feed 2 cats 10min")
            .expect("query")
            .expect_err("error");

        assert_eq!(error.message, multiple_times_message());
    }

    #[test]
    fn parses_reminders_with_required_in_and_to() {
        let action = parse_query("remind in 10 to feed the cat")
            .expect("query")
            .expect("action");
        assert_eq!(action_delay(&action), Duration::from_secs(10));
        assert!(action_title(&action).contains("feed the cat"));

        let action = parse_query("remind me to feed the cat in 10 minutes")
            .expect("query")
            .expect("action");
        assert_eq!(action_delay(&action), Duration::from_secs(600));
        assert!(action_title(&action).contains("feed the cat"));
    }

    #[test]
    fn reminders_require_in_and_to() {
        let error = parse_query("remind me to feed the cat")
            .expect("query")
            .expect_err("error");

        assert_eq!(error.message, reminder_syntax_message());
    }

    #[test]
    fn command_specs_use_system_and_notification_tools() {
        assert_eq!(
            system_action_command(SystemActionKind::Reboot),
            CommandSpec {
                program: "systemctl".into(),
                args: vec!["reboot".into()],
            }
        );
        assert_eq!(
            timer_notification_command("feed the cat"),
            CommandSpec {
                program: "notify-send".into(),
                args: vec![
                    "-i".into(),
                    "stopwatch".into(),
                    "rayslash timer".into(),
                    "feed the cat".into(),
                ],
            }
        );
    }
}
