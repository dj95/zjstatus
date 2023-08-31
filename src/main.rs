use ansi_term::Style;
use config::FormattedPart;
use zellij_tile::prelude::*;

use std::{
    collections::{BTreeMap, HashMap},
    u8,
};

mod config;

#[derive(Default)]
struct State {
    mode_log: HashMap<String, usize>,
    tabs: Vec<String>,
    userspace_configuration: BTreeMap<String, String>,
    module_config: config::ModuleConfig,
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

        self.module_config = config::parse_format(configuration);

        // TODO: current mode
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::ModeUpdate(mode_info) => {
                let mode = format!("{:?}", mode_info.mode);
                let count = self.mode_log.entry(mode).or_insert(0);
                *count += 1;
                should_render = true;
            }
            Event::TabUpdate(tab_info) => {
                self.tabs = tab_info.iter().map(|t| t.name.clone()).collect();
                should_render = true;
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let mut output = "".to_string();

        for part in self.module_config.formatted_parts.to_vec() {
            // TODO: render widgets

            output = format!(
                "{}{}",
                output,
                format!("{}", color(part.clone(), part.content.as_str()))
            );
        }

        println!("{}", output);
    }
}

fn color(part: FormattedPart, text: &str) -> String {
    let mut style = match part.fg {
        Some(color) => Style::new().fg(color),
        None => Style::new(),
    };

    style.background = part.bg;
    style.is_italic = part.italic;
    style.is_bold = part.bold;

    let style = style.paint(text);

    format!("{}", style)
}
