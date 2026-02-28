use std::ops::Sub;

use chrono::{Duration, Local};

use zellij_tile::prelude::PaneManifest;

use crate::{
    config::ZellijState,
    widgets::{command::TIMESTAMP_FORMAT, notification},
};

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
    let lines = input.split('\n').collect::<Vec<&str>>();

    let mut should_render = false;
    for line in lines {
        let line_renders = process_line(state, line);

        if line_renders {
            should_render = true;
        }
    }

    should_render
}

fn resolve_tab_index(panes: &PaneManifest, pane_id: u32) -> Option<usize> {
    for (tab_index, pane_list) in &panes.panes {
        if pane_list.iter().any(|p| p.id == pane_id) {
            return Some(*tab_index);
        }
    }
    None
}

#[tracing::instrument(skip_all)]
fn process_line(state: &mut ZellijState, line: &str) -> bool {
    let parts = line.split("::").collect::<Vec<&str>>();

    if parts.len() < 3 {
        return false;
    }

    if parts[0] != "zjstatus" {
        return false;
    }

    tracing::debug!("command: {}", parts[1]);

    let mut should_render = false;
    #[allow(clippy::single_match)]
    match parts[1] {
        "rerun" => {
            rerun_command(state, parts[2]);

            should_render = true;
        }
        "notify" => {
            notify(state, parts[2]);

            should_render = true;
        }
        "pipe" => {
            if parts.len() < 4 {
                return false;
            }

            pipe(state, parts[2], parts[3]);

            should_render = true;
        }
        "set_status" => {
            if parts.len() < 4 {
                return false;
            }
            let pane_id = match parts[2].parse::<u32>() {
                Ok(id) => id,
                Err(_) => {
                    tracing::warn!("set_status: invalid pane_id: {}", parts[2]);
                    return false;
                }
            };
            let emoji = parts[3];
            if let Some(tab_idx) = resolve_tab_index(&state.panes, pane_id) {
                if emoji.is_empty() {
                    state.tab_statuses.remove(&tab_idx);
                } else {
                    state.tab_statuses.insert(tab_idx, emoji.to_string());
                }
                should_render = true;
            }
        }
        "clear_status" => {
            if parts.len() < 3 {
                return false;
            }
            let pane_id = match parts[2].parse::<u32>() {
                Ok(id) => id,
                Err(_) => {
                    tracing::warn!("clear_status: invalid pane_id: {}", parts[2]);
                    return false;
                }
            };
            if let Some(tab_idx) = resolve_tab_index(&state.panes, pane_id) {
                state.tab_statuses.remove(&tab_idx);
                should_render = true;
            }
        }
        _ => {
            tracing::debug!("unknown zjstatus command: {}", parts[1]);
        }
    }

    should_render
}

fn pipe(state: &mut ZellijState, name: &str, content: &str) {
    tracing::debug!("saving pipe result {name} {content}");
    state
        .pipe_results
        .insert(name.to_owned(), content.to_owned());
}

fn notify(state: &mut ZellijState, message: &str) {
    state.incoming_notification = Some(notification::Message {
        body: message.to_string(),
        received_at: Local::now(),
    });
}

fn rerun_command(state: &mut ZellijState, command_name: &str) {
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
    use std::collections::HashMap;

    use zellij_tile::prelude::{PaneInfo, PaneManifest};

    use crate::config::ZellijState;

    use super::{process_line, resolve_tab_index};

    fn make_state_with_panes() -> ZellijState {
        let mut panes = HashMap::new();
        panes.insert(
            0,
            vec![
                PaneInfo {
                    id: 10,
                    ..PaneInfo::default()
                },
                PaneInfo {
                    id: 11,
                    ..PaneInfo::default()
                },
            ],
        );
        panes.insert(
            1,
            vec![PaneInfo {
                id: 20,
                ..PaneInfo::default()
            }],
        );

        let mut state = ZellijState::default();
        state.panes = PaneManifest { panes };
        state
    }

    #[test]
    fn test_resolve_tab_index_found() {
        let state = make_state_with_panes();
        assert_eq!(resolve_tab_index(&state.panes, 20), Some(1));
    }

    #[test]
    fn test_resolve_tab_index_not_found() {
        let state = make_state_with_panes();
        assert_eq!(resolve_tab_index(&state.panes, 99), None);
    }

    #[test]
    fn test_set_status_valid() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::set_status::10::🤖");
        assert!(result);
        assert_eq!(state.tab_statuses.get(&0), Some(&"🤖".to_string()));
    }

    #[test]
    fn test_set_status_invalid_pane_id() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::set_status::abc::🤖");
        assert!(!result);
        assert!(state.tab_statuses.is_empty());
    }

    #[test]
    fn test_set_status_unknown_pane_id() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::set_status::99::🤖");
        assert!(!result);
        assert!(state.tab_statuses.is_empty());
    }

    #[test]
    fn test_set_status_empty_emoji_clears() {
        let mut state = make_state_with_panes();
        state.tab_statuses.insert(0, "🤖".to_string());
        let result = process_line(&mut state, "zjstatus::set_status::10::");
        assert!(result);
        assert!(state.tab_statuses.get(&0).is_none());
    }

    #[test]
    fn test_clear_status() {
        let mut state = make_state_with_panes();
        state.tab_statuses.insert(1, "✅".to_string());
        let result = process_line(&mut state, "zjstatus::clear_status::20");
        assert!(result);
        assert!(state.tab_statuses.get(&1).is_none());
    }

    #[test]
    fn test_clear_status_idempotent() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::clear_status::20");
        assert!(result);
        assert!(state.tab_statuses.is_empty());
    }

    #[test]
    fn test_set_status_too_few_parts() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::set_status::10");
        assert!(!result);
    }

    #[test]
    fn test_clear_status_invalid_pane_id() {
        let mut state = make_state_with_panes();
        state.tab_statuses.insert(0, "✅".to_string());
        let result = process_line(&mut state, "zjstatus::clear_status::abc");
        assert!(!result);
        assert_eq!(state.tab_statuses.get(&0), Some(&"✅".to_string()));
    }
}
