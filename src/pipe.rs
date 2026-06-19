use std::ops::Sub;

use chrono::{Duration, Local};

use crate::{
    config::ZellijState,
    widgets::{command::TIMESTAMP_FORMAT, notification},
};

pub const DEFAULT_PIPE_OUTPUT_LIMIT_BYTES: usize = 64 * 1024;

/// Parses the line protocol and updates the state accordingly
///
/// The protocol is as follows:
///
/// zjstatus::command_name::args
///
/// It first starts with `zjstatus` as a prefix to indicate that the line is
/// used for the line protocol and zjstatus should parse it. It is followed
/// by the command name and then the arguments. The following commands are
/// available:
///
/// - `rerun` - Reruns the command with the given name (like in the config) as
///             argument. E.g. `zjstatus::rerun::command_1`
///
/// The function returns a boolean indicating whether the state has been
/// changed and the UI should be re-rendered.
#[tracing::instrument(skip(state))]
pub fn parse_protocol(state: &mut ZellijState, input: &str) -> bool {
    tracing::debug!("parsing protocol");

    let mut should_render = false;
    for line in input.split('\n') {
        let line_renders = process_line(state, line);

        if line_renders {
            should_render = true;
        }
    }

    should_render
}

#[tracing::instrument(skip_all)]
fn process_line(state: &mut ZellijState, line: &str) -> bool {
    let mut parts = line.splitn(4, "::");
    let Some(prefix) = parts.next() else {
        return false;
    };
    if prefix != "zjstatus" {
        return false;
    }
    let Some(command) = parts.next() else {
        return false;
    };
    let Some(arg) = parts.next() else {
        return false;
    };

    tracing::debug!("command: {}", command);

    let mut should_render = false;
    #[allow(clippy::single_match)]
    match command {
        "rerun" => {
            rerun_command(state, arg);

            should_render = true;
        }
        "notify" => {
            notify(state, arg);

            should_render = true;
        }
        "pipe" => {
            let Some(content) = parts.next() else {
                return false;
            };

            pipe(state, arg, content);

            should_render = true;
        }
        _ => {}
    }

    should_render
}

fn pipe(state: &mut ZellijState, name: &str, content: &str) {
    tracing::debug!(
        "saving pipe result {name} ({} bytes before limit)",
        content.len()
    );
    state
        .pipe_results
        .insert(name.to_owned(), limit_pipe_content(state, name, content));
}

pub fn pipe_output_limit_from_config(config: &std::collections::BTreeMap<String, String>) -> usize {
    config
        .get("pipe_output_limit_bytes")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PIPE_OUTPUT_LIMIT_BYTES)
}

pub fn pipe_output_limits_from_config(
    config: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, usize> {
    config
        .iter()
        .filter_map(|(key, value)| {
            let pipe_name = key.strip_suffix("_max_bytes")?;
            if !pipe_name.starts_with("pipe_") {
                return None;
            }
            let limit = value.parse::<usize>().ok()?;
            Some((pipe_name.to_owned(), limit))
        })
        .collect()
}

fn limit_pipe_content(state: &ZellijState, name: &str, content: &str) -> String {
    let limit = state
        .pipe_output_limits_bytes
        .get(name)
        .copied()
        .unwrap_or(state.pipe_output_limit_bytes);

    if limit == 0 || content.len() <= limit {
        return content.to_owned();
    }

    let mut start = content.len().saturating_sub(limit);
    while start < content.len() && !content.is_char_boundary(start) {
        start += 1;
    }
    content[start..].to_owned()
}

fn notify(state: &mut ZellijState, message: &str) {
    state.incoming_notification = Some(notification::Message {
        body: message.to_string(),
        received_at: Local::now(),
    });
}

fn rerun_command(state: &mut ZellijState, command_name: &str) {
    invalidate_command_result(state, command_name);
}

/// Backdates the stored timestamp of a command result so the next render
/// re-runs the command.
pub fn invalidate_command_result(state: &mut ZellijState, command_name: &str) {
    let command_result = state.command_results.get(command_name);

    if command_result.is_none() {
        return;
    }

    let mut command_result = command_result.unwrap().clone();

    let ts = Sub::<Duration>::sub(Local::now(), Duration::try_days(1).unwrap());

    command_result.context.insert(
        "timestamp".to_string(),
        ts.format(TIMESTAMP_FORMAT).to_string(),
    );

    state.command_results.remove(command_name);
    state
        .command_results
        .insert(command_name.to_string(), command_result.clone());
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn pipe_protocol_preserves_double_colons_in_content() {
        let mut state = ZellijState {
            pipe_output_limit_bytes: 0,
            ..Default::default()
        };

        assert!(parse_protocol(
            &mut state,
            "zjstatus::pipe::pipe_hints::left::right"
        ));

        assert_eq!(
            state.pipe_results.get("pipe_hints"),
            Some(&"left::right".to_owned())
        );
    }

    #[test]
    fn pipe_protocol_limits_stored_content_to_tail() {
        let mut state = ZellijState {
            pipe_output_limit_bytes: 4,
            ..Default::default()
        };

        assert!(parse_protocol(
            &mut state,
            "zjstatus::pipe::pipe_hints::abcdefghijklmnopqrstuvwxyz"
        ));

        assert_eq!(
            state.pipe_results.get("pipe_hints"),
            Some(&"wxyz".to_owned())
        );
    }

    #[test]
    fn pipe_protocol_limit_keeps_utf8_boundary() {
        let mut state = ZellijState {
            pipe_output_limit_bytes: 6,
            ..Default::default()
        };

        assert!(parse_protocol(
            &mut state,
            "zjstatus::pipe::pipe_hints::a界bcd"
        ));

        assert_eq!(
            state.pipe_results.get("pipe_hints"),
            Some(&"界bcd".to_owned())
        );
    }

    #[test]
    fn pipe_specific_limit_overrides_global_limit() {
        let mut state = ZellijState {
            pipe_output_limit_bytes: 4,
            pipe_output_limits_bytes: BTreeMap::from([("pipe_hints".to_owned(), 2)]),
            ..Default::default()
        };

        assert!(parse_protocol(
            &mut state,
            "zjstatus::pipe::pipe_hints::abcdef"
        ));

        assert_eq!(state.pipe_results.get("pipe_hints"), Some(&"ef".to_owned()));
    }

    #[test]
    fn pipe_output_limits_are_read_from_config() {
        let config = BTreeMap::from([
            ("pipe_output_limit_bytes".to_owned(), "128".to_owned()),
            ("pipe_hints_max_bytes".to_owned(), "16".to_owned()),
        ]);

        assert_eq!(pipe_output_limit_from_config(&config), 128);
        assert_eq!(
            pipe_output_limits_from_config(&config).get("pipe_hints"),
            Some(&16)
        );
    }
}
