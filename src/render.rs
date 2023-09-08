use std::{collections::BTreeMap, sync::Arc};

use ansi_term::{Colour, Colour::Fixed, Colour::RGB, Style};
use zellij_tile::prelude::bail;

use crate::{widgets::widget::Widget, ZellijState};

#[derive(Clone, Debug, PartialEq)]
pub struct FormattedPart {
    pub fg: Option<Colour>,
    pub bg: Option<Colour>,
    pub bold: bool,
    pub italic: bool,
    pub content: String,
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

    pub fn format_string(&self, text: String) -> String {
        let mut style = match self.fg {
            Some(color) => Style::new().fg(color),
            None => Style::new(),
        };

        style.background = self.bg;
        style.is_italic = self.italic;
        style.is_bold = self.bold;

        let style = style.paint(text);

        format!("{}", style)
    }

    pub fn format_string_with_widgets(
        &self,
        widgets: BTreeMap<String, Arc<dyn Widget>>,
        state: ZellijState,
    ) -> String {
        let mut output = self.content.clone();

        for key in widgets.keys() {
            let token = format!("{{{key}}}");
            if !output.contains(token.as_str()) {
                continue;
            }

            let result = match widgets.get(key) {
                Some(widget) => widget.process(state.clone()),
                None => "Use of uninitialized widget".to_string(),
            };

            output = output.replace(token.as_str(), &result);
        }

        self.format_string(output)
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

fn parse_color(color: &str) -> Option<Colour> {
    if color.starts_with('#') {
        let rgb = hex_to_rgb(color.strip_prefix('#').unwrap());
        if rgb.is_err() {
            return None;
        }

        let rgb = rgb.unwrap();
        if rgb.len() != 3 {
            return None;
        }

        return Some(RGB(
            *rgb.first().unwrap(),
            *rgb.get(1).unwrap(),
            *rgb.get(2).unwrap(),
        ));
    }

    if let Ok(result) = color.parse::<u8>() {
        return Some(Fixed(result));
    }

    None
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
        let expected = Colour::RGB(1, 2, 3);
        assert_eq!(result, Some(expected));

        let result = parse_color("255");
        let expected = Colour::Fixed(255);
        assert_eq!(result, Some(expected));

        let result = parse_color("365");
        assert_eq!(result, None);

        let result = parse_color("#365");
        assert_eq!(result, None);
    }
}
