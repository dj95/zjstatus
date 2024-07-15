use std::collections::BTreeMap;

use crate::render::FormattedPart;

const DEFAULT_CHAR: &str = "─";

#[derive(Default, PartialEq, Debug)]
pub enum BorderPosition {
    #[default]
    Top,
    Bottom,
}

#[derive(Debug)]
pub struct BorderConfig {
    pub enabled: bool,
    pub char: String,
    pub format: FormattedPart,
    pub position: BorderPosition,
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            char: DEFAULT_CHAR.to_owned(),
            format: FormattedPart::default(),
            position: BorderPosition::default(),
        }
    }
}

impl BorderConfig {
    pub fn draw(&self, cols: usize) -> String {
        self.format.format_string(&self.char.repeat(cols))
    }
}

pub fn parse_border_config(config: &BTreeMap<String, String>) -> Option<BorderConfig> {
    let enabled = match config.get("border_enabled") {
        Some(e) => matches!(e.as_str(), "true"),
        None => {
            return None;
        }
    };

    let char = match config.get("border_char") {
        Some(bc) => bc,
        None => DEFAULT_CHAR,
    };

    let format = match config.get("border_format") {
        Some(bfs) => bfs,
        None => "",
    };

    let position = match config.get("border_position") {
        Some(pos) => match pos.to_owned().as_str() {
            "bottom" => BorderPosition::Bottom,
            _ => BorderPosition::Top,
        },
        None => BorderPosition::Top,
    };

    Some(BorderConfig {
        enabled,
        char: char.to_owned(),
        format: FormattedPart::from_format_string(format, config),
        position,
    })
}
