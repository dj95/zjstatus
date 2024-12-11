use std::ops::Sub;

use chrono::{Duration, Local};
use zellij_tile::shim::{focus_terminal_pane, hide_self, show_self};

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
        "hide" => {
            process_hide(state, parts[2]);
        }
        "rerun" => {
            rerun_command(state, parts[2]);

            should_render = true;
        }
        "notify" => {
            notify(state, parts[2]);

            should_render = true;
        }
        _ => {}
    }

    should_render
}

fn process_hide(state: &mut ZellijState, message: &str) {
    match message {
        "toggle" => {
            if state.hidden {
                show(state);
            } else {
                hide(state);
            }
        }
        "hide" => {
            hide(state);
        }
        "show" => {
            show(state);
        }
        _ => {}
    }
}

fn hide(state: &mut ZellijState) {
    hide_self();
    state.hidden = true;
}

fn show(state: &mut ZellijState) {
    let focused_pane_id = state
        .panes
        .panes
        .iter()
        .find_map(|(_id, t)| t.iter().find(|a| a.is_focused))
        .unwrap()
        .id;

    show_self(false);
    focus_terminal_pane(focused_pane_id, false);
    state.hidden = false;
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
