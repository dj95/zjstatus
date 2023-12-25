use std::{collections::BTreeMap, str::FromStr};

use chrono::Local;
use chrono_tz::Tz;

use crate::render::FormattedPart;

use crate::{config::ZellijState, widgets::widget::Widget};

pub struct DateTimeWidget {
    format: String,
    color_format: FormattedPart,
    time_zone: Option<Tz>,
}

impl DateTimeWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let mut format = "%H:%M";
        if let Some(form) = config.get("datetime_format") {
            format = form;
        }

        let mut time_zone_string = "Etc/UTC";
        if let Some(tz_string) = config.get("datetime_timezone") {
            time_zone_string = tz_string;
        }

        let time_zone = match Tz::from_str(time_zone_string) {
            Ok(tz) => Some(tz),
            Err(_) => None,
        };

        let mut color_format = "";
        if let Some(form) = config.get("datetime") {
            color_format = form;
        }

        Self {
            format: format.to_owned(),
            color_format: FormattedPart::from_format_string(color_format),
            time_zone,
        }
    }
}

impl Widget for DateTimeWidget {
    fn process(&self, _name: &str ,_state: &ZellijState) -> String {
        let mut output = self.color_format.content.clone();
        if output.contains("{format}") {
            let date = Local::now();

            let mut tz = Tz::UTC;
            if let Some(t) = self.time_zone {
                tz = t;
            }

            output = output.replace(
                "{format}",
                format!("{}", date.with_timezone(&tz).format(self.format.as_str())).as_str(),
            );
        }

        self.color_format.format_string(&output)
    }

    fn process_click(&self, _state: &ZellijState, _pos: usize) {}
}
