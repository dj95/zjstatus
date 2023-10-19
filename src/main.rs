use config::ModuleConfig;
use widgets::{
    command::{CommandResult, CommandWidget},
    datetime::DateTimeWidget,
    mode::ModeWidget,
    session::SessionWidget,
    swap_layout::SwapLayoutWidget,
    tabs::TabsWidget,
    widget::Widget,
};
use zellij_tile::prelude::*;

use chrono::{DateTime, Local};
use std::{collections::BTreeMap, sync::Arc, usize};
use uuid::Uuid;

mod border;
mod config;
mod frames;
mod render;
mod widgets;

#[derive(Default)]
struct State {
    state: ZellijState,
    userspace_configuration: BTreeMap<String, String>,
    module_config: config::ModuleConfig,
    widget_map: BTreeMap<String, Arc<dyn Widget>>,
    got_permissions: bool,
}

#[derive(Default, Debug, Clone)]
pub struct ZellijState {
    pub cols: usize,
    pub command_results: BTreeMap<String, CommandResult>,
    pub mode: ModeInfo,
    pub plugin_uuid: String,
    pub tabs: Vec<TabInfo>,
    pub sessions: Vec<SessionInfo>,
    pub start_time: DateTime<Local>,
}

#[cfg(not(test))]
register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // we need the ReadApplicationState permission to receive the ModeUpdate and TabUpdate
        // events
        // we need the RunCommands permission to run "cargo test" in a floating window
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::RunCommands,
        ]);
        subscribe(&[
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
            EventType::TabUpdate,
            EventType::SessionUpdate,
            EventType::RunCommandResult,
        ]);

        self.userspace_configuration = configuration.clone();
        self.module_config = ModuleConfig::new(configuration.clone());
        self.widget_map = register_widgets(configuration);
        self.got_permissions = false;
        let uid = Uuid::new_v4();

        self.state = ZellijState {
            cols: 0,
            command_results: BTreeMap::new(),
            mode: ModeInfo::default(),
            plugin_uuid: uid.to_string(),
            tabs: Vec::new(),
            sessions: Vec::new(),
            start_time: Local::now(),
        };
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::Mouse(mouse_info) => {
                if !self.got_permissions {
                    return false;
                }

                self.module_config.handle_mouse_action(
                    self.state.clone(),
                    mouse_info,
                    self.widget_map.clone(),
                );
            }
            Event::ModeUpdate(mode_info) => {
                if !self.got_permissions {
                    return false;
                }

                self.state.mode = mode_info;
                should_render = true;
            }
            Event::PaneUpdate(pane_info) => {
                if !self.got_permissions {
                    return false;
                }

                if self.module_config.hide_frame_for_single_pane {
                    frames::hide_frames_on_single_pane(
                        self.state.tabs.clone(),
                        pane_info,
                        get_plugin_ids(),
                    );

                    should_render = true;
                }
            }
            Event::PermissionRequestResult(result) => {
                eprintln!("{:?}", result);
                set_selectable(false);
                self.got_permissions = true;
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                if !self.got_permissions {
                    return false;
                }

                if let Some(name) = context.get("name") {
                    let stdout = match String::from_utf8(stdout) {
                        Ok(s) => s,
                        Err(_) => "".to_owned(),
                    };

                    let stderr = match String::from_utf8(stderr) {
                        Ok(s) => s,
                        Err(_) => "".to_owned(),
                    };

                    self.state.command_results.insert(
                        name.to_string(),
                        CommandResult {
                            exit_code,
                            stdout,
                            stderr,
                            context,
                        },
                    );
                }
            }
            Event::SessionUpdate(session_info) => {
                if !self.got_permissions {
                    return false;
                }

                if self.module_config.hide_frame_for_single_pane {
                    let current_session = session_info.iter().find(|s| s.is_current_session);

                    if let Some(current_session) = current_session {
                        frames::hide_frames_on_single_pane(
                            current_session.clone().tabs,
                            current_session.clone().panes,
                            get_plugin_ids(),
                        );
                    }
                }

                self.state.sessions = session_info;

                should_render = true;
            }
            Event::TabUpdate(tab_info) => {
                if !self.got_permissions {
                    return false;
                }

                self.state.tabs = tab_info;
                should_render = true;
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, _rows: usize, cols: usize) {
        if is_detached(self.state.clone()) || !self.got_permissions {
            return;
        }

        self.state.cols = cols;

        self.module_config
            .render_bar(self.state.clone(), self.widget_map.clone());
    }
}

fn is_detached(state: ZellijState) -> bool {
    let current_session = match state.sessions.iter().find(|s| s.is_current_session) {
        Some(s) => s,
        None => return false,
    };

    return current_session.connected_clients == 0;
}

fn register_widgets(configuration: BTreeMap<String, String>) -> BTreeMap<String, Arc<dyn Widget>> {
    let mut widget_map = BTreeMap::<String, Arc<dyn Widget>>::new();

    widget_map.insert(
        "command".to_string(),
        Arc::new(CommandWidget::new(configuration.clone())),
    );
    widget_map.insert(
        "datetime".to_string(),
        Arc::new(DateTimeWidget::new(configuration.clone())),
    );
    widget_map.insert(
        "swap_layout".to_string(),
        Arc::new(SwapLayoutWidget::new(configuration.clone())),
    );
    widget_map.insert(
        "mode".to_string(),
        Arc::new(ModeWidget::new(configuration.clone())),
    );
    widget_map.insert(
        "session".to_string(),
        Arc::new(SessionWidget::new(configuration.clone())),
    );
    widget_map.insert("tabs".to_string(), Arc::new(TabsWidget::new(configuration)));

    widget_map
}
