use std::collections::BTreeMap;

use ansi_term::{Colour, Colour::Fixed};

#[derive(Default)]
pub struct ModuleConfig {
    pub enabled_modules: Vec<String>,
    pub formatted_parts: Vec<FormattedPart>,
}

#[derive(Clone, Debug)]
pub struct FormattedPart {
    pub order: u8,
    pub fg: Option<Colour>,
    pub bg: Option<Colour>,
    pub bold: bool,
    pub italic: bool,
    pub content: String,
}

fn parse_color(color: &str) -> Option<Colour> {
    if color.starts_with("#") {
        return Some(Fixed(1));
    }

    if let Ok(result) = color.parse::<u8>() {
        return Some(Fixed(result));
    }

    None
}

impl FormattedPart {
    pub fn from_format_string(format: String) -> Self {
        let mut result = FormattedPart::default();

        let format_content_split = format.split("]");

        if format_content_split.clone().count() == 1 {
            result.content = format;

            return result;
        }

        let format_content_split = format_content_split.collect::<Vec<&str>>();
        result.content = format_content_split[1].to_string();

        let parts = format_content_split[0].split(",");
        for part in parts {
            if part.starts_with("fg=") {
                result.fg = parse_color(part.strip_prefix("fg=").unwrap());
            }

            if part.starts_with("bg=") {
                result.bg = parse_color(part.strip_prefix("bg=").unwrap());
            }

            if part.eq("bold") {
                result.bold = true;
            }

            if part.eq("italic") {
                result.italic = true;
            }
        }

        result
    }
}

impl Default for FormattedPart {
    fn default() -> Self {
        Self {
            order: 0,
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            content: "".to_string(),
        }
    }
}

pub fn parse_format(config: BTreeMap<String, String>) -> ModuleConfig {
    let format = config.get("format");
    let mut formatted_parts = Vec::new();

    if format.is_none() {
        return ModuleConfig {
            enabled_modules: Vec::new(),
            formatted_parts,
        };
    }

    let format = format.unwrap();

    let mut counter: u8 = 0;
    let color_parts = format.split("#[");
    for color_part in color_parts {
        let mut part = FormattedPart::from_format_string(color_part.to_string());
        part.order = counter.clone();

        formatted_parts.push(part);

        counter += 1;
    }

    ModuleConfig {
        enabled_modules: Vec::new(),
        formatted_parts,
    }
}
