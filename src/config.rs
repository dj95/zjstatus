use std::collections::BTreeMap;

use ansi_term::{Colour, Colour::Fixed, Colour::RGB};

#[derive(Default)]
pub struct ModuleConfig {
    pub left_parts: Vec<FormattedPart>,
    pub right_parts: Vec<FormattedPart>,
    pub format_space: FormattedPart,
    pub hide_frame_for_single_pane: bool,
}

#[derive(Clone, Debug)]
pub struct FormattedPart {
    pub fg: Option<Colour>,
    pub bg: Option<Colour>,
    pub bold: bool,
    pub italic: bool,
    pub content: String,
}

fn parse_color(color: &str) -> Option<Colour> {
    if color.starts_with('#') {
        let rgb = hex_rgb::convert_hexcode_to_rgb(color.to_string()).unwrap();

        return Some(RGB(rgb.red, rgb.green, rgb.blue));
    }

    if let Ok(result) = color.parse::<u8>() {
        return Some(Fixed(result));
    }

    None
}

impl FormattedPart {
    pub fn from_format_string(format: String) -> Self {
        let mut format = format;
        if format.starts_with("#[") {
            format = format.strip_prefix("#[").unwrap().to_string();
        }
        let mut result = FormattedPart::default();

        let format_content_split = format.split(']');

        if format_content_split.clone().count() == 1 {
            result.content = format;

            return result;
        }

        let format_content_split = format_content_split.collect::<Vec<&str>>();
        result.content = format_content_split[1].to_string();

        let parts = format_content_split[0].split(',');
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
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            content: "".to_string(),
        }
    }
}

pub fn parse_format(config: BTreeMap<String, String>) -> ModuleConfig {
    let mut format_space_config = "";
    if let Some(space_config) = config.get("format_space") {
        format_space_config = space_config;
    }

    let mut hide_frame_for_single_pane = false;
    if let Some(toggle) = config.get("hide_frame_for_single_pane") {
        hide_frame_for_single_pane = toggle == "true";
    }

    ModuleConfig {
        left_parts: parts_from_config(config.get("format_left")),
        right_parts: parts_from_config(config.get("format_right")),
        format_space: FormattedPart::from_format_string(format_space_config.to_string()),
        hide_frame_for_single_pane,
    }
}

fn parts_from_config(format: Option<&String>) -> Vec<FormattedPart> {
    if format.is_none() {
        return Vec::new();
    }

    let mut output = Vec::new();

    let format_left = format.unwrap();

    let color_parts = format_left.split("#[");
    for color_part in color_parts {
        let part = FormattedPart::from_format_string(color_part.to_string());

        output.push(part);
    }

    output
}
