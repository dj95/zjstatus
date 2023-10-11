use std::collections::BTreeMap;

use zellij_tile::{prelude::TabInfo, shim::switch_tab_to};

use crate::render::FormattedPart;

use super::widget::Widget;

pub struct TabsWidget {
    active_tab_format: Vec<FormattedPart>,
    active_tab_fullscreen_format: Vec<FormattedPart>,
    active_tab_sync_format: Vec<FormattedPart>,
    normal_tab_format: Vec<FormattedPart>,
    normal_tab_fullscreen_format: Vec<FormattedPart>,
    normal_tab_sync_format: Vec<FormattedPart>,
}

impl TabsWidget {
    pub fn new(config: BTreeMap<String, String>) -> Self {
        let mut normal_tab_format: Vec<FormattedPart> = Vec::new();
        if let Some(form) = config.get("tab_normal") {
            normal_tab_format = get_formatters(form.to_string());
        }

        let normal_tab_fullscreen_format = match config.get("tab_normal_fullscreen") {
            Some(form) => get_formatters(form.to_string()),
            None => normal_tab_format.clone(),
        };

        let normal_tab_sync_format = match config.get("tab_normal_sync") {
            Some(form) => get_formatters(form.to_string()),
            None => normal_tab_format.clone(),
        };

        let mut active_tab_format = normal_tab_format.clone();
        if let Some(form) = config.get("tab_active") {
            active_tab_format = get_formatters(form.to_string());
        }

        let active_tab_fullscreen_format = match config.get("tab_active_fullscreen") {
            Some(form) => get_formatters(form.to_string()),
            None => active_tab_format.clone(),
        };

        let active_tab_sync_format = match config.get("tab_active_sync") {
            Some(form) => get_formatters(form.to_string()),
            None => active_tab_format.clone(),
        };

        Self {
            normal_tab_format,
            normal_tab_fullscreen_format,
            normal_tab_sync_format,
            active_tab_format,
            active_tab_fullscreen_format,
            active_tab_sync_format,
        }
    }
}

impl Widget for TabsWidget {
    fn process(&self, state: crate::ZellijState) -> String {
        let mut output = "".to_string();

        for tab in state.tabs {
            let content = self.render_tab(tab);

            output = format!("{}{}", output, content);
        }

        output
    }

    fn process_click(&self, state: crate::ZellijState, pos: usize) {
        let mut output = "".to_string();

        let mut offset = 0;
        let mut index = 1;
        for tab in state.tabs {
            let content = strip_ansi_escapes::strip_str(self.render_tab(tab));

            if pos > offset && pos < offset + content.chars().count() {
                switch_tab_to(index);

                break;
            }

            index += 1;
            offset += content.chars().count();

            output = format!("{}{}", output, content);
        }
    }
}

impl TabsWidget {
    fn select_format(&self, info: TabInfo) -> Vec<FormattedPart> {
        if info.active && info.is_fullscreen_active {
            return self.active_tab_fullscreen_format.clone();
        }

        if info.active && info.is_sync_panes_active {
            return self.active_tab_sync_format.clone();
        }

        if info.active {
            return self.active_tab_format.clone();
        }

        if info.is_fullscreen_active {
            return self.normal_tab_fullscreen_format.clone();
        }

        if info.is_sync_panes_active {
            return self.normal_tab_sync_format.clone();
        }

        self.normal_tab_format.clone()
    }

    fn render_tab(&self, tab: TabInfo) -> String {
        let formatters = self.select_format(tab.clone());
        let mut output = "".to_string();

        for f in formatters.iter() {
            let mut content = f.content.clone();

            if content.contains("{name}") {
                content = content.replace("{name}", tab.name.as_str());
            }

            if content.contains("{index}") {
                content = content.replace("{index}", (tab.position + 1).to_string().as_str());
            }

            output = format!("{}{}", output, f.format_string(content));
        }

        output.to_string()
    }
}

fn get_formatters(config: String) -> Vec<FormattedPart> {
    let mut output = Vec::new();

    let color_parts = config.split("#[");
    for color_part in color_parts {
        let part = FormattedPart::from_format_string(color_part.to_string());

        output.push(part);
    }

    output
}
