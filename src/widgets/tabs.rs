use std::collections::BTreeMap;

use zellij_tile::{prelude::TabInfo, shim::switch_tab_to};

use crate::render::FormattedPart;

use super::widget::Widget;

pub struct TabsWidget {
    active_tab_format: FormattedPart,
    active_tab_fullscreen_format: FormattedPart,
    active_tab_sync_format: FormattedPart,
    normal_tab_format: FormattedPart,
    normal_tab_fullscreen_format: FormattedPart,
    normal_tab_sync_format: FormattedPart,
}

impl TabsWidget {
    pub fn new(config: BTreeMap<String, String>) -> Self {
        let mut normal_tab_format_string = "";
        if let Some(form) = config.get("tab_normal") {
            normal_tab_format_string = form;
        }

        let normal_tab_fullscreen_format =
            FormattedPart::from_format_string(match config.get("tab_normal_fullscreen") {
                Some(form) => form.to_string(),
                None => normal_tab_format_string.to_string(),
            });

        let normal_tab_sync_format =
            FormattedPart::from_format_string(match config.get("tab_normal_sync") {
                Some(form) => form.to_string(),
                None => normal_tab_format_string.to_string(),
            });

        let mut active_tab_format_string = normal_tab_format_string.clone();
        if let Some(form) = config.get("tab_active") {
            active_tab_format_string = form;
        }

        let active_tab_fullscreen_format =
            FormattedPart::from_format_string(match config.get("tab_active_fullscreen") {
                Some(form) => form.to_string(),
                None => active_tab_format_string.to_string(),
            });

        let active_tab_sync_format =
            FormattedPart::from_format_string(match config.get("tab_active_sync") {
                Some(form) => form.to_string(),
                None => active_tab_format_string.to_string(),
            });

        Self {
            normal_tab_format: FormattedPart::from_format_string(
                normal_tab_format_string.to_string(),
            ),
            normal_tab_fullscreen_format,
            normal_tab_sync_format,
            active_tab_format: FormattedPart::from_format_string(
                active_tab_format_string.to_string(),
            ),
            active_tab_fullscreen_format,
            active_tab_sync_format,
        }
    }
}

impl Widget for TabsWidget {
    fn process(&self, state: crate::ZellijState) -> String {
        let mut output = "".to_string();

        for tab in state.tabs {
            let formatter = self.select_format(tab.clone());
            let content = self.render_tab(tab);

            output = format!("{}{}", output, formatter.format_string(content),);
        }

        output
    }

    fn process_click(&self, state: crate::ZellijState, pos: usize) {
        let mut output = "".to_string();

        let mut offset = 0;
        let mut index = 1;
        for tab in state.tabs {
            let content = self.render_tab(tab);

            if pos > offset && pos < offset + content.len() {
                switch_tab_to(index);

                break;
            }

            index += 1;
            offset += content.len();

            output = format!("{}{}", output, content);
        }
    }
}

impl TabsWidget {
    fn select_format(&self, info: TabInfo) -> FormattedPart {
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
        let formatter = self.select_format(tab.clone());

        let mut content = formatter.content.clone();
        if content.contains("{name}") {
            content = content.replace("{name}", tab.name.as_str());
        }

        if content.contains("{index}") {
            content = content.replace("{index}", (tab.position + 1).to_string().as_str());
        }

        content
    }
}
