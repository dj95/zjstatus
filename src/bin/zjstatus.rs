use zellij_tile::prelude::*;

use chrono::Local;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use uuid::Uuid;

use zjstatus::{
    config::{self, ModuleConfig, UpdateEventMask, ZellijState},
    frames, pipe, tab_groups,
    widgets::{
        attention::AttentionWidget,
        command::{CommandResult, CommandWidget},
        datetime::DateTimeWidget,
        mode::ModeWidget,
        notification::NotificationWidget,
        pipe::PipeWidget,
        session::SessionWidget,
        swap_layout::SwapLayoutWidget,
        tabs::TabsWidget,
        widget::Widget,
    },
};

// Matches the old incidental Zellij session scan cadence.
const REFRESH_INTERVAL_SECONDS: f64 = 1.0;

#[derive(Default)]
struct State {
    pending_events: Vec<Event>,
    got_permissions: bool,
    state: ZellijState,
    userspace_configuration: BTreeMap<String, String>,
    module_config: config::ModuleConfig,
    widget_map: BTreeMap<String, Arc<dyn Widget>>,
    focus_cwd_commands: Vec<String>,
    err: Option<anyhow::Error>,
    tab_groups: tab_groups::Controller,
}

#[cfg(not(test))]
register_plugin!(State);

#[cfg(feature = "tracing")]
fn init_tracing() {
    use std::fs::File;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let file = File::create("/host/.zjstatus.log");
    let file = match file {
        Ok(file) => file,
        Err(error) => panic!("Error: {:?}", error),
    };
    let debug_log = tracing_subscriber::fmt::layer().with_writer(Arc::new(file));

    tracing_subscriber::registry().with(debug_log).init();

    tracing::info!("tracing initialized");
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        #[cfg(feature = "tracing")]
        init_tracing();

        // we need the ReadApplicationState permission to receive the ModeUpdate and TabUpdate
        // events
        // we need the RunCommands permission to run "cargo test" in a floating window
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::RunCommands,
            // Tab groups need it to move tabs. Requested whatever the config: Zellij replaces the
            // saved grant for the plugin URL with each instance's requested set, so every bar
            // must ask for the same set or the grant flips between bars.
            PermissionType::RunActionsAsUser,
        ]);

        subscribe(&[
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
            EventType::Timer,
            EventType::TabUpdate,
            EventType::SessionUpdate,
            EventType::RunCommandResult,
            EventType::CwdChanged,
        ]);
        set_timeout(REFRESH_INTERVAL_SECONDS);

        self.module_config = match ModuleConfig::new(&configuration) {
            Ok(mc) => mc,
            Err(e) => {
                self.err = Some(e);
                return;
            }
        };
        self.widget_map = register_widgets(&configuration);
        self.focus_cwd_commands =
            zjstatus::widgets::command::focus_cwd_command_names(&configuration);
        self.tab_groups =
            tab_groups::Controller::new(&configuration, get_plugin_ids().plugin_id).asking_zellij();
        self.userspace_configuration = configuration;
        self.pending_events = Vec::new();
        self.got_permissions = false;
        let uid = Uuid::new_v4();

        self.state = ZellijState {
            cols: 0,
            command_results: BTreeMap::new(),
            pipe_results: BTreeMap::new(),
            mode: ModeInfo::default(),
            panes: PaneManifest::default(),
            plugin_uuid: uid.to_string(),
            tabs: Vec::new(),
            sessions: Vec::new(),
            start_time: Local::now(),
            cache_mask: 0,
            incoming_notification: None,
            focused_pane_id: None,
            focused_pane_cwd: None,
            tab_groups: Default::default(),
        };
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        let groups_changed = self.handle_tab_group_pipe(&pipe_message);
        let mut should_render = false;

        match pipe_message.source {
            PipeSource::Cli(_) => {
                if let Some(input) = pipe_message.payload {
                    should_render = pipe::parse_protocol(&mut self.state, &input);
                }
            }
            PipeSource::Plugin(_) => {
                if let Some(input) = pipe_message.payload {
                    should_render = pipe::parse_protocol(&mut self.state, &input);
                }
            }
            PipeSource::Keybind => {
                if let Some(input) = pipe_message.payload {
                    should_render = pipe::parse_protocol(&mut self.state, &input);
                }
            }
            PipeSource::PromptRequest { .. } => {}
        }

        should_render || groups_changed
    }

    #[tracing::instrument(skip_all, fields(event_type))]
    fn update(&mut self, event: Event) -> bool {
        if let Event::PermissionRequestResult(PermissionStatus::Granted) = event {
            self.got_permissions = true;

            while !self.pending_events.is_empty() {
                tracing::debug!("processing cached event");
                let ev = self.pending_events.pop();

                self.handle_event(ev.unwrap());
            }
        }

        if !self.got_permissions {
            tracing::debug!("caching event");
            self.pending_events.push(event);

            return false;
        }

        self.handle_event(event)
    }

    #[tracing::instrument(skip_all)]
    fn render(&mut self, _rows: usize, cols: usize) {
        if !self.got_permissions {
            return;
        }

        if let Some(err) = &self.err {
            println!("Error: {:?}", err);

            return;
        }

        self.state.cols = cols;

        tracing::debug!("{:?}", self.state.mode.session_name);

        let output = self
            .module_config
            .render_bar(self.state.clone(), self.widget_map.clone());

        print!("{}", output);
    }
}

impl State {
    /// Handles `zjstatus::groups::...` messages. Returns whether the bar
    /// changed.
    fn handle_tab_group_pipe(&mut self, pipe_message: &PipeMessage) -> bool {
        let changed = pipe_message
            .payload
            .as_deref()
            .is_some_and(|input| self.tab_groups.handle_pipe(&mut self.state, input));
        if changed {
            self.state.cache_mask = UpdateEventMask::Tab as u8;
        }
        changed
    }

    /// Reads the shared tab group state again. Returns whether it changed.
    fn reload_tab_groups(&mut self) -> bool {
        let changed = self.tab_groups.reload(&mut self.state);
        if changed {
            self.state.cache_mask = UpdateEventMask::Tab as u8;
        }
        changed
    }

    fn update_focused_pane(&mut self) {
        let active_tab = self.state.tabs.iter().find(|t| t.active);

        let new_id = active_tab
            .and_then(|tab| self.state.panes.panes.get(&tab.position))
            .and_then(|panes| panes.iter().find(|p| p.is_focused && !p.is_plugin))
            .map(|p| PaneId::Terminal(p.id));

        if new_id == self.state.focused_pane_id {
            return;
        }

        self.state.focused_pane_id = new_id;

        let new_cwd = match new_id {
            Some(pane_id) => match get_pane_cwd(pane_id) {
                Ok(cwd) => Some(cwd),
                Err(e) => {
                    tracing::debug!("could not get pane cwd: {e}");
                    None
                }
            },
            None => None,
        };

        self.set_focused_pane_cwd(new_cwd);
    }

    fn set_focused_pane_cwd(&mut self, new_cwd: Option<PathBuf>) -> bool {
        if new_cwd == self.state.focused_pane_cwd {
            return false;
        }

        self.state.focused_pane_cwd = new_cwd;

        if self.focus_cwd_commands.is_empty() {
            return false;
        }

        self.invalidate_focus_cwd_commands();
        true
    }

    fn invalidate_focus_cwd_commands(&mut self) {
        for name in &self.focus_cwd_commands {
            pipe::invalidate_command_result(&mut self.state, name);
            zjstatus::widgets::command::release_command_lock(&self.state, name);
        }
    }

    fn handle_event(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::Mouse(mouse_info) => {
                tracing::Span::current().record("event_type", "Event::Mouse");
                tracing::debug!(mouse = ?mouse_info);

                self.state.tab_groups.left_click = matches!(mouse_info, Mouse::LeftClick(..));
                self.module_config.handle_mouse_action(
                    self.state.clone(),
                    mouse_info,
                    self.widget_map.clone(),
                );
                if self.state.tab_groups.left_click {
                    should_render = self.reload_tab_groups();
                }
            }
            Event::ModeUpdate(mode_info) => {
                tracing::Span::current().record("event_type", "Event::ModeUpdate");
                tracing::debug!(mode = ?mode_info.mode);
                tracing::debug!(mode = ?mode_info.session_name);

                self.state.mode = mode_info;
                self.state.cache_mask = UpdateEventMask::Mode as u8;
                self.reload_tab_groups();

                should_render = true;
            }
            Event::PaneUpdate(pane_info) => {
                tracing::Span::current().record("event_type", "Event::PaneUpdate");
                tracing::debug!(pane_count = ?pane_info.panes.len());

                frames::hide_frames_conditionally(
                    &frames::FrameConfig::new(
                        self.module_config.hide_frame_for_single_pane,
                        self.module_config.hide_frame_except_for_search,
                        self.module_config.hide_frame_except_for_fullscreen,
                        self.module_config.hide_frame_except_for_scroll,
                        &self.module_config.pane_frame_style,
                    ),
                    &self.state.tabs,
                    &pane_info,
                    &self.state.mode,
                    get_plugin_ids(),
                    false,
                );

                self.state.panes = pane_info;
                self.state.cache_mask = UpdateEventMask::Tab as u8;

                self.update_focused_pane();

                should_render = true;
            }
            Event::CwdChanged(pane_id, cwd, _clients) => {
                tracing::Span::current().record("event_type", "Event::CwdChanged");
                tracing::debug!(pane_id = ?pane_id, cwd = ?cwd);

                if Some(pane_id) == self.state.focused_pane_id
                    && self.set_focused_pane_cwd(Some(cwd))
                {
                    self.state.cache_mask = UpdateEventMask::Command as u8;
                    should_render = true;
                }
            }
            Event::PermissionRequestResult(result) => {
                tracing::Span::current().record("event_type", "Event::PermissionRequestResult");
                tracing::debug!(result = ?result);
                set_selectable(false);
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                tracing::Span::current().record("event_type", "Event::RunCommandResult");
                tracing::debug!(
                    exit_code = ?exit_code,
                    stdout = ?String::from_utf8(stdout.clone()),
                    stderr = ?String::from_utf8(stderr.clone()),
                    context = ?context
                );

                self.state.cache_mask = UpdateEventMask::Command as u8;

                if let Some(name) = context.get("name") {
                    if self.focus_cwd_commands.iter().any(|n| n == name)
                        && context.get("cwd").map(PathBuf::from) != self.state.focused_pane_cwd
                    {
                        tracing::debug!("discarding stale command result for {name}");
                        return false;
                    }

                    let stdout = match String::from_utf8(stdout) {
                        Ok(s) => s,
                        Err(_) => "".to_owned(),
                    };

                    let stderr = match String::from_utf8(stderr) {
                        Ok(s) => s,
                        Err(_) => "".to_owned(),
                    };

                    self.state.command_results.insert(
                        name.to_owned(),
                        CommandResult {
                            exit_code,
                            stdout,
                            stderr,
                            context,
                        },
                    );
                }
            }
            Event::SessionUpdate(session_info, _) => {
                tracing::Span::current().record("event_type", "Event::SessionUpdate");

                let current_session = session_info.iter().find(|s| s.is_current_session);

                if let Some(current_session) = current_session {
                    frames::hide_frames_conditionally(
                        &frames::FrameConfig::new(
                            self.module_config.hide_frame_for_single_pane,
                            self.module_config.hide_frame_except_for_search,
                            self.module_config.hide_frame_except_for_fullscreen,
                            self.module_config.hide_frame_except_for_scroll,
                            &self.module_config.pane_frame_style,
                        ),
                        &current_session.tabs,
                        &current_session.panes,
                        &self.state.mode,
                        get_plugin_ids(),
                        false,
                    );
                }

                self.state.sessions = session_info;
                self.state.cache_mask = UpdateEventMask::Session as u8;

                should_render = true;
            }
            Event::TabUpdate(tab_info) => {
                tracing::Span::current().record("event_type", "Event::TabUpdate");
                tracing::debug!(tab_count = ?tab_info.len());

                self.state.cache_mask = UpdateEventMask::Tab as u8;
                self.state.tabs = tab_info;
                self.reload_tab_groups();
                self.tab_groups.keep_together(&self.state);

                should_render = true;
            }
            Event::Timer(_) => {
                tracing::Span::current().record("event_type", "Event::Timer");
                set_timeout(REFRESH_INTERVAL_SECONDS);
                self.state.cache_mask = 0;
                // Another instance may have expanded or collapsed a group.
                self.reload_tab_groups();
                self.tab_groups.keep_together(&self.state);

                should_render = true;
            }
            _ => (),
        };
        should_render
    }
}

fn register_widgets(configuration: &BTreeMap<String, String>) -> BTreeMap<String, Arc<dyn Widget>> {
    let mut widget_map = BTreeMap::<String, Arc<dyn Widget>>::new();

    widget_map.insert(
        "command".to_owned(),
        Arc::new(CommandWidget::new(configuration)),
    );
    widget_map.insert(
        "datetime".to_owned(),
        Arc::new(DateTimeWidget::new(configuration)),
    );
    widget_map.insert("pipe".to_owned(), Arc::new(PipeWidget::new(configuration)));
    widget_map.insert(
        "swap_layout".to_owned(),
        Arc::new(SwapLayoutWidget::new(configuration)),
    );
    widget_map.insert("mode".to_owned(), Arc::new(ModeWidget::new(configuration)));
    widget_map.insert(
        "session".to_owned(),
        Arc::new(SessionWidget::new(configuration)),
    );
    widget_map.insert("tabs".to_owned(), Arc::new(TabsWidget::new(configuration)));
    widget_map.insert(
        "notifications".to_owned(),
        Arc::new(NotificationWidget::new(configuration)),
    );
    widget_map.insert(
        "attention".to_owned(),
        Arc::new(AttentionWidget::new(configuration)),
    );

    tracing::debug!("registered widgets: {:?}", widget_map.keys());

    widget_map
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    /// A bar with groups, in the active tab of `api: a`, `api: b`.
    fn grouped_bar() -> State {
        let configuration = BTreeMap::from([("tab_groups".to_owned(), "true".to_owned())]);
        let mut bar = State {
            tab_groups: tab_groups::Controller::new(&configuration, 7),
            ..State::default()
        };
        bar.state.tabs = ["api: a", "api: b"]
            .iter()
            .enumerate()
            .map(|(position, name)| TabInfo {
                position,
                tab_id: position,
                name: name.to_string(),
                active: position == 0,
                ..TabInfo::default()
            })
            .collect();
        let own_pane = PaneInfo {
            id: 7,
            is_plugin: true,
            ..PaneInfo::default()
        };
        bar.state.panes.panes = HashMap::from([(0, vec![own_pane])]);
        bar.state.mode.session_name = Some("test".to_owned());
        bar
    }

    fn message(source: PipeSource, payload: &str) -> PipeMessage {
        PipeMessage::new(source, "zjstatus", &Some(payload.to_owned()), &None, false)
    }

    /// Regression: the upstream messages' result overwrote the group change, so
    /// a keybinding's toggle showed only at the next Zellij event.
    #[test]
    fn a_group_command_redraws_the_bar_whatever_its_source() {
        for source in [
            PipeSource::Keybind,
            PipeSource::Cli("pipe-id".to_owned()),
            PipeSource::Plugin(1),
        ] {
            let mut bar = grouped_bar();

            assert!(
                bar.pipe(message(source.clone(), "zjstatus::groups::toggle")),
                "{source:?}"
            );
            assert!(bar.state.tab_groups.collapse.is_collapsed("api", false));
            assert_eq!(bar.state.cache_mask, UpdateEventMask::Tab as u8);
        }
    }

    #[test]
    fn other_messages_redraw_only_if_they_change_something() {
        let mut bar = grouped_bar();

        assert!(bar.pipe(message(PipeSource::Keybind, "zjstatus::notify::hello")));
        assert!(!bar.pipe(message(PipeSource::Keybind, "something else")));
    }

    #[test]
    fn set_focused_pane_cwd_only_invalidates_on_change() {
        let mut state = State {
            focus_cwd_commands: vec!["command_branch".to_owned()],
            ..State::default()
        };
        state.state.focused_pane_cwd = Some(PathBuf::from("/tmp"));
        state.state.command_results.insert(
            "command_branch".to_owned(),
            CommandResult {
                context: BTreeMap::from([(
                    "timestamp".to_owned(),
                    Local::now()
                        .format(zjstatus::widgets::command::TIMESTAMP_FORMAT)
                        .to_string(),
                )]),
                ..CommandResult::default()
            },
        );

        let original_timestamp =
            state.state.command_results["command_branch"].context["timestamp"].clone();

        assert!(!state.set_focused_pane_cwd(Some(PathBuf::from("/tmp"))));
        assert_eq!(
            state.state.command_results["command_branch"].context["timestamp"],
            original_timestamp
        );

        assert!(state.set_focused_pane_cwd(Some(PathBuf::from("/var"))));
        assert_eq!(state.state.focused_pane_cwd, Some(PathBuf::from("/var")));
        assert_ne!(
            state.state.command_results["command_branch"].context["timestamp"],
            original_timestamp
        );
    }
}
