use lazy_static::lazy_static;
use std::{collections::BTreeMap, sync::Arc};

use anstyle::{Ansi256Color, AnsiColor, Color, RgbColor, Style};
use regex::Regex;
use zellij_tile::prelude::bail;

use crate::{config::ZellijState, widgets::widget::Widget};

lazy_static! {
    static ref WIDGET_REGEX: Regex = Regex::new("(\\{[a-z_0-9]+\\})").unwrap();
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormattedPart {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub content: String,
}

impl FormattedPart {
    pub fn multiple_from_format_string(config: String) -> Vec<Self> {
        config
            .split("#[")
            .map(|c| FormattedPart::from_format_string(c.to_string()))
            .collect()
    }

    pub fn from_format_string(format: String) -> Self {
        let format = match format.starts_with("#[") {
            true => format.strip_prefix("#[").unwrap().to_string(),
            false => format,
        };

        let mut result = FormattedPart::default();

        let format_content_split = format.split(']');

        if format_content_split.clone().count() == 1 {
            result.content = format;

            return result;
        }

        let mut format_content_split = format_content_split.collect::<Vec<&str>>();
        let parts = format_content_split[0].split(',');

        format_content_split.remove(0);
        result.content = format_content_split.join("]");

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

    pub fn format_string(&self, text: &str) -> String {
        let mut style = Style::new();

        style = style.fg_color(self.fg);
        style = style.bg_color(self.bg);

        if self.italic {
            style = style.italic();
        }

        if self.bold {
            style = style.bold();
        }

        format!(
            "{}{}{}{}",
            style.render_reset(),
            style.render(),
            text,
            style.render_reset()
        )
    }

    pub fn format_string_with_widgets(
        &self,
        widgets: &BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
    ) -> String {
        let mut output = self.content.clone();

        for widget in WIDGET_REGEX.captures_iter(&self.content) {
            let match_name = widget.get(0).unwrap().as_str();
            let mut widget_key = match_name.trim_matches(|c| c == '{' || c == '}');

            if widget_key.starts_with("command_") {
                widget_key = "command";
            }

            let result = match widgets.get(widget_key) {
                Some(widget) => widget.process(
                    match_name.trim_matches(|c| c == '{' || c == '}'),
                    state,
                ),
                None => "Use of uninitialized widget".to_string(),
            };

            output = output.replace(match_name, &result);
        }

        self.format_string(&output)
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

fn hex_to_rgb(s: &str) -> anyhow::Result<Vec<u8>> {
    if s.len() != 6 {
        bail!("wrong hex color length");
    }

    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(anyhow::Error::from))
        .collect()
}

fn parse_color(color: &str) -> Option<Color> {
    if color.starts_with('#') {
        let rgb = match hex_to_rgb(color.strip_prefix('#').unwrap()) {
            Ok(rgb) => rgb,
            Err(_) => return None,
        };

        if rgb.len() != 3 {
            return None;
        }

        return Some(
            RgbColor(
                *rgb.first().unwrap(),
                *rgb.get(1).unwrap(),
                *rgb.get(2).unwrap(),
            )
            .into(),
        );
    }

    if let Some(color) = color_by_name(color) {
        return Some(color.into());
    }

    if let Ok(result) = color.parse::<u8>() {
        return Some(Ansi256Color(result).into());
    }

    None
}

fn color_by_name(color: &str) -> Option<AnsiColor> {
    match color {
        "black" => Some(AnsiColor::Black),
        "red" => Some(AnsiColor::Red),
        "green" => Some(AnsiColor::Green),
        "yellow" => Some(AnsiColor::Yellow),
        "blue" => Some(AnsiColor::Blue),
        "magenta" => Some(AnsiColor::Magenta),
        "cyan" => Some(AnsiColor::Cyan),
        "white" => Some(AnsiColor::White),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_hex_to_rgb() {
        let result = hex_to_rgb("010203");
        let expected = Vec::from([1, 2, 3]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_hex_to_rgb_with_invalid_input() {
        let result = hex_to_rgb("#010203");
        assert!(result.is_err());

        let result = hex_to_rgb(" 010203");
        assert!(result.is_err());

        let result = hex_to_rgb("010");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_color() {
        let result = parse_color("#010203");
        let expected = RgbColor(1, 2, 3);
        assert_eq!(result, Some(expected.into()));

        let result = parse_color("255");
        let expected = Ansi256Color(255);
        assert_eq!(result, Some(expected.into()));

        let result = parse_color("365");
        assert_eq!(result, None);

        let result = parse_color("#365");
        assert_eq!(result, None);
    }
}
