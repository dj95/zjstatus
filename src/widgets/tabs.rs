use std::collections::BTreeMap;

use zellij_tile::{prelude::TabInfo, shim::switch_tab_to};

use crate::render::FormattedPart;

use super::widget::Widget;

pub struct TabsWidget {
    active_tab_format: FormattedPart,
    normal_tab_format: FormattedPart,
}

impl TabsWidget {
    pub fn new(config: BTreeMap<String, String>) -> Self {
        let mut normal_tab_format_string = "";
        if let Some(form) = config.get("tab_normal") {
            normal_tab_format_string = form;
        }

        let active_tab_format = FormattedPart::from_format_string(match config.get("tab_active") {
            Some(form) => form.to_string(),
            None => normal_tab_format_string.to_string(),
        });

        Self {
            normal_tab_format: FormattedPart::from_format_string(
                normal_tab_format_string.to_string(),
            ),
            active_tab_format,
        }
    }
}

impl Widget for TabsWidget {
    fn process(&self, state: crate::ZellijState) -> String {
        let mut output = "".to_string();

        for tab in state.tabs {
            let formatter = self.select_format(tab.clone());

            let mut content = formatter.content.clone();
            if content.contains("{name}") {
                content = content.replace("{name}", tab.name.as_str());
            }

            output = format!("{}{}", output, formatter.format_string(content),);
        }

        output
    }

    fn process_click(&self, state: crate::ZellijState, pos: usize) {
        let mut output = "".to_string();

        let mut offset = 0;
        let mut index = 1;
        for tab in state.tabs {
            let formatter = self.select_format(tab.clone());

            let mut content = formatter.content.clone();
            if content.contains("{name}") {
                content = content.replace("{name}", tab.name.as_str());
            }
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
        match info.active {
            false => self.normal_tab_format.clone(),
            true => self.active_tab_format.clone(),
        }
    }
}
