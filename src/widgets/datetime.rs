use std::collections::BTreeMap;

use chrono::Local;

use crate::{config::FormattedPart, render};

use super::widget::Widget;

pub struct DateTimeWidget {
    format: String,
    color_format: FormattedPart,
}

impl DateTimeWidget {
    pub fn new(config: BTreeMap<String, String>) -> Self {
        let mut format = "%H:%M";
        if let Some(form) = config.get("datetime_format") {
            format = form;
        }

        let mut color_format = "";
        if let Some(form) = config.get("datetime") {
            color_format = form;
        }

        Self {
            format: format.to_string(),
            color_format: FormattedPart::from_format_string(color_format.to_string()),
        }
    }
}

impl Widget for DateTimeWidget {
    fn process(&self, _state: crate::ZellijState) -> String {
        let mut output = self.color_format.content.clone();
        if output.contains("{format}") {
            let date = Local::now();

            output = output.replace(
                "{format}",
                format!("{}", date.format(self.format.as_str())).as_str(),
            );
        }

        render::formatting(self.color_format.clone(), output)
    }
}
