use std::collections::BTreeMap;

use chrono::Local;

use crate::{config::FormattedPart, render};

use super::widget::Widget;

pub struct ClockWidget {
    format: String,
    color_format: FormattedPart,
}

impl ClockWidget {
    pub fn new(config: BTreeMap<String, String>) -> Self {
        let mut format = "%H:%M";
        if let Some(form) = config.get("clock_format") {
            format = form;
        }

        let mut color_format = "";
        if let Some(form) = config.get("clock") {
            color_format = form;
        }

        Self {
            format: format.to_string(),
            color_format: FormattedPart::from_format_string(color_format.to_string()),
        }
    }
}

impl Widget for ClockWidget {
    fn process(&self, _state: crate::ZellijState) -> String {
        let mut output = self.color_format.content.clone();
        if output.contains("{time}") {
            let date = Local::now();

            output = output.replace(
                "{time}",
                format!("{}", date.format(self.format.as_str())).as_str(),
            );
        }

        render::formatting(self.color_format.clone(), output)
    }
}
