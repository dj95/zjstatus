use cached::{proc_macro::cached, SizedCache};
use lazy_static::lazy_static;
use std::{collections::BTreeMap, sync::Arc};

use anstyle::{Ansi256Color, AnsiColor, Color, RgbColor, Style};
use regex::Regex;
use zellij_tile::prelude::bail;

use crate::{
    config::{event_mask_from_widget_name, UpdateEventMask, ZellijState},
    widgets::widget::Widget,
};

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
    pub cache_mask: u8,
    pub cached_content: String,
    pub cache: BTreeMap<String, String>,
}

#[cached(
    type = "SizedCache<String, FormattedPart>",
    create = "{ SizedCache::with_size(100) }",
    convert = r#"{ (format.to_owned()) }"#
)]
pub fn formatted_part_from_string_cached(format: &str) -> FormattedPart {
    FormattedPart::from_format_string(format)
}

#[cached(
    type = "SizedCache<String, Vec<FormattedPart>>",
    create = "{ SizedCache::with_size(100) }",
    convert = r#"{ (config.to_owned()) }"#
)]
pub fn formatted_parts_from_string_cached(config: &str) -> Vec<FormattedPart> {
    FormattedPart::multiple_from_format_string(config)
}

impl FormattedPart {
    pub fn multiple_from_format_string(config: &str) -> Vec<Self> {
        config
            .split("#[")
            .map(FormattedPart::from_format_string)
            .collect()
    }

    pub fn from_format_string(format: &str) -> Self {
        let format = match format.starts_with("#[") {
            true => format.strip_prefix("#[").unwrap(),
            false => format,
        };

        let mut result = FormattedPart {
            cache_mask: cache_mask_from_content(format),
            ..Default::default()
        };

        let mut format_content_split = format.split(']').collect::<Vec<&str>>();

        if format_content_split.len() == 1 {
            result.content = format.to_owned();

            return result;
        }

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
        &mut self,
        widgets: &BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
    ) -> String {
        let skip_cache = self.cache_mask & UpdateEventMask::Always as u8 != 0;
        if !skip_cache && self.cache_mask & state.cache_mask == 0 && !self.cache.is_empty() {
            return self.cached_content.to_owned();
        }

        let mut output = self.content.clone();

        for widget in WIDGET_REGEX.captures_iter(&self.content) {
            let match_name = widget.get(0).unwrap().as_str();
            let widget_key = match_name.trim_matches(|c| c == '{' || c == '}');
            let mut widget_key_name = widget_key;

            if widget_key.starts_with("command_") {
                widget_key_name = "command";
            }

            let widget_mask = event_mask_from_widget_name(widget_key_name);
            if !skip_cache && widget_mask & state.cache_mask == 0 {
                if let Some(res) = self.cache.get(widget_key) {
                    output = output.replace(match_name, res);
                    continue;
                }
            }

            let result = match widgets.get(widget_key_name) {
                Some(widget) => widget.process(widget_key, state),
                None => "Use of uninitialized widget".to_owned(),
            };

            self.cache.insert(widget_key.to_owned(), result.to_owned());

            output = output.replace(match_name, &result);
        }

        let res = self.format_string(&output);
        self.cached_content = res.clone();

        res
    }
}

impl Default for FormattedPart {
    fn default() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            content: "".to_owned(),
            cache_mask: 0,
            cached_content: "".to_owned(),
            cache: BTreeMap::new(),
        }
    }
}

fn cache_mask_from_content(content: &str) -> u8 {
    let mut output = 0;
    for widget in WIDGET_REGEX.captures_iter(content) {
        let match_name = widget.get(0).unwrap().as_str();
        let widget_key = match_name.trim_matches(|c| c == '{' || c == '}');
        let mut widget_key_name = widget_key;

        if widget_key.starts_with("command_") {
            widget_key_name = "command";
        }

        output |= event_mask_from_widget_name(widget_key_name);
    }
    output
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

#[cached(
    type = "SizedCache<String, Option<Color>>",
    create = "{ SizedCache::with_size(100) }",
    convert = r#"{ (color.to_owned()) }"#
)]
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
