use widgets::{mode::ModeWidget, widget::Widget};
use zellij_tile::prelude::*;

use std::{
    collections::BTreeMap,
    sync::Arc,
    u8, usize,
};

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
    pub tabs: Vec<String>,
    pub rows: usize,
    pub cols: usize,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.userspace_configuration = configuration.clone();
        // we need the ReadApplicationState permission to receive the ModeUpdate and TabUpdate
        // events
        // we need the RunCommands permission to run "cargo test" in a floating window
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::RunCommands,
        ]);
        subscribe(&[EventType::ModeUpdate, EventType::TabUpdate, EventType::Key]);

        self.module_config = config::parse_format(configuration.clone());

        let mut widget_map = BTreeMap::<String, Arc<dyn Widget>>::new();

        let mode_widget = ModeWidget::new(configuration);
        widget_map.insert("mode".to_string(), Arc::new(mode_widget));

        self.widget_map = widget_map;

        self.state = ZellijState {
            mode: ModeInfo::default(),
            tabs: Vec::new(),
            rows: 0,
            cols: 0,
        };
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::ModeUpdate(mode_info) => {
                self.state.mode = mode_info;
                should_render = true;
            }
            Event::TabUpdate(tab_info) => {
                self.state.tabs = tab_info.iter().map(|t| t.name.clone()).collect();
                should_render = true;
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let mut output = "".to_string();

        self.state.rows = rows;
        self.state.cols = cols;

        for part in self.module_config.formatted_parts.to_vec() {
            output = format!(
                "{}{}",
                output,
                format!(
                    "{}",
                    render::widgets_and_formatting(
                        part,
                        self.widget_map.clone(),
                        self.state.clone()
                    )
                )
            );
        }

        println!("{}", output);
    }
}
