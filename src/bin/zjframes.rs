use zellij_tile::prelude::*;

use std::{collections::BTreeMap, sync::Arc};

use zjstatus::frames;

#[derive(Default, Debug, Clone)]
pub struct ZellijState {
    pub mode: ModeInfo,
    pub panes: PaneManifest,
    pub tabs: Vec<TabInfo>,
}

#[derive(Default)]
struct State {
    pending_events: Vec<Event>,
    got_permissions: bool,
    state: ZellijState,

    hide_frame_for_single_pane: bool,
    hide_frame_except_for_search: bool,
    hide_frame_except_for_fullscreen: bool,
    hide_frame_except_for_scroll: bool,

    err: Option<anyhow::Error>,
}

#[cfg(not(test))]
register_plugin!(State);

#[cfg(feature = "tracing")]
fn init_tracing() {
    use std::fs::File;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let file = File::create("/host/.zjframes.log");
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
        ]);

        subscribe(&[
            EventType::ModeUpdate,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
            EventType::TabUpdate,
            EventType::SessionUpdate,
        ]);

        self.hide_frame_for_single_pane = match configuration.get("hide_frame_for_single_pane") {
            Some(toggle) => toggle == "true",
            None => false,
        };
        self.hide_frame_except_for_search = match configuration.get("hide_frame_except_for_search")
        {
            Some(toggle) => toggle == "true",
            None => false,
        };
        self.hide_frame_except_for_fullscreen =
            match configuration.get("hide_frame_except_for_fullscreen") {
                Some(toggle) => toggle == "true",
                None => false,
            };
        self.hide_frame_except_for_scroll=
            match configuration.get("hide_frame_except_for_scroll") {
                Some(toggle) => toggle == "true",
                None => false,
            };

        self.pending_events = Vec::new();
        self.got_permissions = false;
        self.state = ZellijState::default();
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
    fn render(&mut self, _rows: usize, _cols: usize) {
        if !self.got_permissions {
            return;
        }

        if let Some(err) = &self.err {
            println!("Error: {:?}", err);

            return;
        }

        print!("Please load this plugin in the background");
    }
}

impl State {
    fn handle_event(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::ModeUpdate(mode_info) => {
                tracing::Span::current().record("event_type", "Event::ModeUpdate");
                tracing::debug!(mode = ?mode_info.mode);
                tracing::debug!(mode = ?mode_info.session_name);

                self.state.mode = mode_info;

                frames::hide_frames_conditionally(
                    &frames::FrameConfig::new(
                        self.hide_frame_for_single_pane,
                        self.hide_frame_except_for_search,
                        self.hide_frame_except_for_fullscreen,
                        self.hide_frame_except_for_scroll,
                    ),
                    &self.state.tabs,
                    &self.state.panes,
                    &self.state.mode,
                    get_plugin_ids(),
                    true,
                );
            }
            Event::PaneUpdate(pane_info) => {
                tracing::Span::current().record("event_type", "Event::PaneUpdate");
                tracing::debug!(pane_count = ?pane_info.panes.len());

                self.state.panes = pane_info;

                frames::hide_frames_conditionally(
                    &frames::FrameConfig::new(
                        self.hide_frame_for_single_pane,
                        self.hide_frame_except_for_search,
                        self.hide_frame_except_for_fullscreen,
                        self.hide_frame_except_for_scroll,
                    ),
                    &self.state.tabs,
                    &self.state.panes,
                    &self.state.mode,
                    get_plugin_ids(),
                    true,
                );
            }
            Event::PermissionRequestResult(result) => {
                tracing::Span::current().record("event_type", "Event::PermissionRequestResult");
                tracing::debug!(result = ?result);
                set_selectable(false);
                should_render = true;
            }
            Event::SessionUpdate(session_info, _) => {
                tracing::Span::current().record("event_type", "Event::SessionUpdate");

                let current_session = session_info.iter().find(|s| s.is_current_session);

                if let Some(current_session) = current_session {
                    frames::hide_frames_conditionally(
                        &frames::FrameConfig::new(
                            self.hide_frame_for_single_pane,
                            self.hide_frame_except_for_search,
                            self.hide_frame_except_for_fullscreen,
                            self.hide_frame_except_for_scroll,
                        ),
                        &current_session.tabs,
                        &current_session.panes,
                        &self.state.mode,
                        get_plugin_ids(),
                        true,
                    );
                }
            }
            Event::TabUpdate(tab_info) => {
                tracing::Span::current().record("event_type", "Event::TabUpdate");
                tracing::debug!(tab_count = ?tab_info.len());

                self.state.tabs = tab_info;
            }
            _ => (),
        };
        should_render
    }
}
