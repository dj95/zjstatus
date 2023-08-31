use widgets::{mode::ModeWidget, tabs::TabsWidget, widget::Widget, datetime::DateTimeWidget};
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
        self.widget_map = register_widgets(configuration);

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
                self.state.tabs = tab_info;
                should_render = true;
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.state.rows = rows.clone();
        self.state.cols = cols.clone();

        let mut output_left = "".to_string();
        for part in self.module_config.left_parts.to_vec() {
            output_left = format!(
                "{}{}",
                output_left,
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

        let mut output_right = "".to_string();
        for part in self.module_config.right_parts.to_vec() {
            output_right = format!(
                "{}{}",
                output_right,
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

        let left_count = strip_ansi_escapes::strip(output_left.clone()).len();
        let right_count = strip_ansi_escapes::strip(output_right.clone()).len();

        let spacer = " ".repeat(cols - left_count - right_count);

        println!("{}{}{}", output_left, spacer, output_right);
    }
}

fn register_widgets(configuration: BTreeMap<String, String>) -> BTreeMap<String, Arc<dyn Widget>> {
    let mut widget_map = BTreeMap::<String, Arc<dyn Widget>>::new();

    widget_map.insert("datetime".to_string(), Arc::new(DateTimeWidget::new(configuration.clone())));
    widget_map.insert(
        "mode".to_string(),
        Arc::new(ModeWidget::new(configuration.clone())),
    );
    widget_map.insert("tabs".to_string(), Arc::new(TabsWidget::new(configuration)));

    widget_map
}
