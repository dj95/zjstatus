use widgets::{
    datetime::DateTimeWidget, mode::ModeWidget, session::SessionWidget, tabs::TabsWidget,
    widget::Widget,
};
use zellij_tile::prelude::*;

use std::{collections::BTreeMap, sync::Arc, u8, usize};

mod config;
mod render;
mod widgets;

#[derive(Default)]
struct State {
    state: ZellijState,
    userspace_configuration: BTreeMap<String, String>,
    module_config: config::ModuleConfig,
    widget_map: BTreeMap<String, Arc<dyn Widget>>,
}

#[derive(Default, Debug, Clone)]
pub struct ZellijState {
    pub mode: ModeInfo,
    pub tabs: Vec<TabInfo>,
    pub sessions: Vec<SessionInfo>,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // we need the ReadApplicationState permission to receive the ModeUpdate and TabUpdate
        // events
        // we need the RunCommands permission to run "cargo test" in a floating window
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::RunCommands,
        ]);
        subscribe(&[
            EventType::ModeUpdate,
            EventType::TabUpdate,
            EventType::SessionUpdate,
        ]);

        let mut selectable = false;
        if let Some(first_start) = configuration.get("first_start") {
            if first_start.eq("true") {
                selectable = true;
            }
        }

        set_selectable(selectable);

        self.userspace_configuration = configuration.clone();
        self.module_config = config::parse_format(configuration.clone());
        self.widget_map = register_widgets(configuration);

        self.state = ZellijState {
            mode: ModeInfo::default(),
            tabs: Vec::new(),
            sessions: Vec::new(),
        };
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::ModeUpdate(mode_info) => {
                self.state.mode = mode_info;
                should_render = true;
            }
            Event::SessionUpdate(session_info) => {
                self.state.sessions = session_info;
                should_render = true;
            }
            Event::TabUpdate(tab_info) => {
                self.state.tabs = tab_info;
                should_render = true;
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, _rows: usize, cols: usize) {
        let mut output_left = "".to_string();
        for part in self.module_config.left_parts.iter().cloned() {
            output_left = format!(
                "{}{}",
                output_left,
                render::widgets_and_formatting(part, self.widget_map.clone(), self.state.clone())
            );
        }

        let mut output_right = "".to_string();
        for part in self.module_config.right_parts.iter().cloned() {
            output_right = format!(
                "{}{}",
                output_right,
                render::widgets_and_formatting(part, self.widget_map.clone(), self.state.clone())
            );
        }

        let text_count = strip_ansi_escapes::strip(output_left.clone()).len()
            + strip_ansi_escapes::strip(output_right.clone()).len();

        let mut space_count = cols;
        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        if space_count > text_count {
            space_count -= text_count;
        }

        let spaces = render::formatting(
            self.module_config.format_space.clone(),
            " ".repeat(space_count),
        );

        print!("{}{}{}", output_left, spaces, output_right);
    }
}

fn register_widgets(configuration: BTreeMap<String, String>) -> BTreeMap<String, Arc<dyn Widget>> {
    let mut widget_map = BTreeMap::<String, Arc<dyn Widget>>::new();

    widget_map.insert(
        "datetime".to_string(),
        Arc::new(DateTimeWidget::new(configuration.clone())),
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
