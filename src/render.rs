use cached::{LruCache, macros::cached};
use lazy_static::lazy_static;
use std::{collections::BTreeMap, sync::Arc};

use anstyle::{Ansi256Color, AnsiColor, Color, RgbColor, Style};
use regex::Regex;
use zellij_tile::prelude::bail;

use crate::{
    config::{UpdateEventMask, ZellijState, event_mask_from_widget_name},
    widgets::widget::Widget,
};

lazy_static! {
    static ref WIDGET_REGEX: Regex = Regex::new("(\\{[a-z_0-9]+\\})").unwrap();
}

/// Which session role a segment configured with `only_when=` renders for.
/// A segment with no `only_when` attribute renders for both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    Host,
    Nested,
}

impl Role {
    fn from_config_value(value: &str) -> Option<Self> {
        match value {
            "host" => Some(Role::Host),
            "nested" => Some(Role::Nested),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormattedPart {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub us: Option<Color>,
    pub effects: anstyle::Effects,
    pub bold: bool,
    pub italic: bool,
    pub underscore: bool,
    pub reverse: bool,
    pub blink: bool,
    pub hidden: bool,
    pub dimmed: bool,
    pub strikethrough: bool,
    pub double_underscore: bool,
    pub curly_underscore: bool,
    pub dotted_underscore: bool,
    pub dashed_underscore: bool,
    pub only_when: Option<Role>,
    pub content: String,
    pub cache_mask: u8,
    pub cached_content: String,
    pub cache: BTreeMap<String, String>,
}

#[cached(
    ty = "LruCache<String, FormattedPart>",
    create = "{ LruCache::builder().max_size(100).build().unwrap() }",
    convert = r#"{ (format.to_owned()) }"#
)]
pub fn formatted_part_from_string_cached(
    format: &str,
    config: &BTreeMap<String, String>,
) -> FormattedPart {
    FormattedPart::from_format_string(format, config)
}

#[cached(
    ty = "LruCache<String, Vec<FormattedPart>>",
    create = "{ LruCache::builder().max_size(100).build().unwrap() }",
    convert = r#"{ (config_string.to_owned()) }"#
)]
pub fn formatted_parts_from_string_cached(
    config_string: &str,
    config: &BTreeMap<String, String>,
) -> Vec<FormattedPart> {
    FormattedPart::multiple_from_format_string(config_string, config)
}

impl FormattedPart {
    pub fn multiple_from_format_string(
        config_string: &str,
        config: &BTreeMap<String, String>,
    ) -> Vec<Self> {
        config_string
            .split("#[")
            .map(|s| FormattedPart::from_format_string(s, config))
            .collect()
    }

    pub fn from_format_string(format: &str, config: &BTreeMap<String, String>) -> Self {
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
            format.clone_into(&mut result.content);

            return result;
        }

        let parts = format_content_split[0].split(',');

        format_content_split.remove(0);
        result.content = format_content_split.join("]");

        for part in parts {
            if part.starts_with("fg=") {
                result.fg = parse_color(part.strip_prefix("fg=").unwrap(), config);
            }

            if part.starts_with("bg=") {
                result.bg = parse_color(part.strip_prefix("bg=").unwrap(), config);
            }

            if part.starts_with("us=") {
                result.us = parse_color(part.strip_prefix("us=").unwrap(), config);
            }

            if let Some(role) = part.strip_prefix("only_when=") {
                result.only_when = Role::from_config_value(role);
            }

            if part.eq("reverse") {
                result.reverse = true;
            }

            result.parse_and_set_effect(part);
        }

        result
    }

    fn parse_and_set_effect(&mut self, part: &str) {
        match part {
            "bold" => {
                self.effects |= anstyle::Effects::BOLD;
            }
            "italic" | "italics" => {
                self.effects |= anstyle::Effects::ITALIC;
            }
            "underscore" => {
                self.effects |= anstyle::Effects::UNDERLINE;
            }
            "blink" => {
                self.effects |= anstyle::Effects::BLINK;
            }
            "hidden" => {
                self.effects |= anstyle::Effects::HIDDEN;
            }
            "dim" => {
                self.effects |= anstyle::Effects::DIMMED;
            }
            "strikethrough" => {
                self.effects |= anstyle::Effects::STRIKETHROUGH;
            }
            "double-underscore" => {
                self.effects |= anstyle::Effects::DOUBLE_UNDERLINE;
            }
            "curly-underscore" => {
                self.effects |= anstyle::Effects::CURLY_UNDERLINE;
            }
            "dotted-underscore" => {
                self.effects |= anstyle::Effects::DOTTED_UNDERLINE;
            }
            "dashed-underscore" => {
                self.effects |= anstyle::Effects::DASHED_UNDERLINE;
            }
            "reverse" => {
                self.effects |= anstyle::Effects::INVERT;
            }
            _ => {}
        }
    }

    /// Renders `text` with this part's configured styling. `dim` is a
    /// strength in `0.0..=1.0`; `0.0` renders normally, anything above that
    /// blends `fg`/`bg`/`us` toward neutral gray by that fraction (see
    /// `dim_color`). Callers derive it from `ZellijState::dim_amount`, which
    /// is `0.0` unless this session is currently the dimmed side of a
    /// nested-session pair.
    pub fn format_string(&self, text: &str, dim: f32) -> String {
        let mut style = Style::new();

        let (fg, bg, us) = if dim > 0.0 {
            (
                self.fg.map(|c| dim_color(c, dim)),
                self.bg.map(|c| dim_color(c, dim)),
                self.us.map(|c| dim_color(c, dim)),
            )
        } else {
            (self.fg, self.bg, self.us)
        };

        style = style.fg_color(fg);
        style = style.bg_color(bg);
        style = style.underline_color(us);
        style = style.effects(self.effects);

        format!(
            "{}{}{}{}",
            style.render_reset(),
            style.render(),
            text,
            style.render_reset()
        )
    }

    /// Whether this part should render at all given the session's current
    /// host/nested role. A part with no `only_when` attribute always
    /// matches; `session_ancestry` being non-empty means this session is
    /// nested inside another one. A nested session whose pane currently
    /// fills its host's entire screen (`host_fullscreen`) is treated as a
    /// host too: it covers the whole outer session, so it renders the
    /// same as a standalone one rather than showing nested-only chrome.
    fn role_matches(&self, state: &ZellijState) -> bool {
        let is_host =
            state.mode.session_ancestry.is_empty() || state.mode.host_fullscreen == Some(true);

        match self.only_when {
            None => true,
            Some(Role::Host) => is_host,
            Some(Role::Nested) => !is_host,
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn format_string_with_widgets(
        &mut self,
        widgets: &BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
    ) -> String {
        if !self.role_matches(state) {
            return String::new();
        }

        let skip_cache = self.cache_mask & UpdateEventMask::Always as u8 != 0;

        if !skip_cache && self.cache_mask & state.cache_mask == 0 && !self.cache.is_empty() {
            tracing::debug!(msg = "hit", typ = "format_string", format = self.content);
            return self.cached_content.to_owned();
        }
        tracing::debug!(msg = "miss", typ = "format_string", format = self.content);

        let mut output = self.content.clone();

        for widget in WIDGET_REGEX.captures_iter(&self.content) {
            let match_name = widget.get(0).unwrap().as_str();
            let widget_key = match_name.trim_matches(|c| c == '{' || c == '}');
            let mut widget_key_name = widget_key;

            if widget_key.starts_with("command_") {
                widget_key_name = "command";
            }

            if widget_key.starts_with("pipe_") {
                widget_key_name = "pipe";
            }

            let widget_mask = event_mask_from_widget_name(widget_key_name);
            let skip_widget_cache = widget_mask & UpdateEventMask::Always as u8 != 0;
            if !skip_widget_cache
                && widget_mask & state.cache_mask == 0
                && let Some(res) = self.cache.get(widget_key)
            {
                tracing::debug!(msg = "hit", typ = "widget", widget = widget_key);
                output = output.replace(match_name, res);
                continue;
            }

            tracing::debug!(
                msg = "miss",
                typ = "widget",
                widget = widget_key,
                mask = widget_mask & state.cache_mask,
                skip_cache = skip_cache,
            );

            let result = match widgets.get(widget_key_name) {
                Some(widget) => widget.process(widget_key, state),
                None => "Use of uninitialized widget".to_owned(),
            };

            self.cache.insert(widget_key.to_owned(), result.to_owned());

            output = output.replace(match_name, &result);
        }

        let res = self.format_string(&output, state.dim_amount());
        self.cached_content.clone_from(&res);

        res
    }
}

impl Default for FormattedPart {
    fn default() -> Self {
        Self {
            fg: None,
            bg: None,
            us: None,
            effects: anstyle::Effects::new(),
            bold: false,
            italic: false,
            underscore: false,
            reverse: false,
            blink: false,
            hidden: false,
            dimmed: false,
            strikethrough: false,
            double_underscore: false,
            curly_underscore: false,
            dotted_underscore: false,
            dashed_underscore: false,
            only_when: None,
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

        if widget_key.starts_with("pipe_") {
            widget_key_name = "pipe";
        }

        output |= event_mask_from_widget_name(widget_key_name);
    }
    output
}

/// A fully dimmed color sits at this fraction of its own brightness: dimmed
/// chrome should read as visibly darker, not just the same brightness with
/// the hue washed out.
const DIM_BRIGHTNESS: f32 = 0.3;

/// Fades an RGB color toward a dark, desaturated gray by `strength` (`0.0` =
/// unchanged, `1.0` = fully dimmed). Each channel moves toward
/// `DIM_BRIGHTNESS` of the color's own perceived luminance, rather than
/// toward a fixed midpoint or toward the color's own unchanged brightness:
/// the former reads as a contrast/brightness shift more than a fade for
/// colors far from that midpoint, and the latter fades out hue without
/// getting any darker, so dimmed chrome doesn't visually recede the way
/// core's own dimmed pane chrome does. This blends both at once: a color
/// fades out its hue while also darkening, ending at a dim neutral gray
/// rather than just getting flatter. Named ANSI colors and 256-color
/// palette entries pass through unchanged: they're indices into a
/// terminal-defined palette, not RGB triples, so there's no well-defined
/// "dimmed version" of one to compute without also assuming a specific
/// palette.
fn dim_color(color: Color, strength: f32) -> Color {
    match color {
        Color::Rgb(RgbColor(r, g, b)) => {
            // ITU-R BT.601 luma weights: the standard "how bright does this
            // color look" formula used by most grayscale conversions.
            let luminance = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
            let target = luminance * DIM_BRIGHTNESS;

            let blend = |channel: u8| -> u8 {
                let channel = channel as f32;
                (channel + (target - channel) * strength)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            Color::Rgb(RgbColor(blend(r), blend(g), blend(b)))
        }
        other => other,
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

#[cached(
    ty = "LruCache<String, Color>",
    create = "{ LruCache::builder().max_size(100).build().unwrap() }",
    convert = r#"{ (color.to_owned()) }"#
)]
fn parse_color(color: &str, config: &BTreeMap<String, String>) -> Option<Color> {
    let mut color = color;
    if color.starts_with('$') {
        let alias_name = color.strip_prefix('$').unwrap();

        color = config.get(&format!("color_{alias_name}"))?;
    }

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

    if color.starts_with("colour") {
        color = color.strip_prefix("colour").unwrap();
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
        "bright_black" => Some(AnsiColor::BrightBlack),
        "bright_red" => Some(AnsiColor::BrightRed),
        "bright_green" => Some(AnsiColor::BrightGreen),
        "bright_yellow" => Some(AnsiColor::BrightYellow),
        "bright_blue" => Some(AnsiColor::BrightBlue),
        "bright_magenta" => Some(AnsiColor::BrightMagenta),
        "bright_cyan" => Some(AnsiColor::BrightCyan),
        "bright_white" => Some(AnsiColor::BrightWhite),
        "default" => None,
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
        let mut config: BTreeMap<String, String> = BTreeMap::new();
        config.insert("color_green".to_owned(), "#00ff00".to_owned());

        let result = parse_color("#010203", &config);
        let expected = RgbColor(1, 2, 3);
        assert_eq!(result, Some(expected.into()));

        let result = parse_color("255", &config);
        let expected = Ansi256Color(255);
        assert_eq!(result, Some(expected.into()));

        let result = parse_color("365", &config);
        assert_eq!(result, None);

        let result = parse_color("#365", &config);
        assert_eq!(result, None);

        let result = parse_color("$green", &config);
        let expected = RgbColor(0, 255, 0);
        assert_eq!(result, Some(expected.into()));

        let result = parse_color("$blue", &config);
        assert_eq!(result, None);
    }

    #[test]
    fn test_dim_color_blends_rgb_toward_gray() {
        // Pure red's BT.601 luminance is 0.299*255 = 76.245; the dim target
        // is 30% of that (~22.9), so full strength converges there on all
        // three channels: darker than the color's own brightness, not just
        // desaturated to it.
        let red = Color::Rgb(RgbColor(255, 0, 0));
        assert_eq!(dim_color(red, 0.0), red);
        assert_eq!(dim_color(red, 1.0), Color::Rgb(RgbColor(23, 23, 23)));
        assert_eq!(dim_color(red, 0.5), Color::Rgb(RgbColor(139, 11, 11)));

        // A color that's already gray has nothing to desaturate, but still
        // darkens: its luminance equals every channel already, but the dim
        // target is a fraction of that.
        let white = Color::Rgb(RgbColor(255, 255, 255));
        assert_eq!(dim_color(white, 1.0), Color::Rgb(RgbColor(77, 77, 77)));
    }

    #[test]
    fn test_dim_color_passes_through_indexed_colors() {
        let ansi = Color::Ansi(AnsiColor::Red);
        let ansi256 = Color::Ansi256(Ansi256Color(200));
        assert_eq!(dim_color(ansi, 0.8), ansi);
        assert_eq!(dim_color(ansi256, 0.8), ansi256);
    }

    #[test]
    fn test_only_when_parses_from_format_string() {
        let host = FormattedPart::from_format_string("#[only_when=host]foo", &BTreeMap::new());
        assert_eq!(host.only_when, Some(Role::Host));

        let nested = FormattedPart::from_format_string("#[only_when=nested]foo", &BTreeMap::new());
        assert_eq!(nested.only_when, Some(Role::Nested));

        let unset = FormattedPart::from_format_string("#[fg=#ff0000]foo", &BTreeMap::new());
        assert_eq!(unset.only_when, None);

        let invalid = FormattedPart::from_format_string("#[only_when=bogus]foo", &BTreeMap::new());
        assert_eq!(invalid.only_when, None);
    }

    #[test]
    fn test_role_matches() {
        let host_part = FormattedPart {
            only_when: Some(Role::Host),
            ..Default::default()
        };
        let nested_part = FormattedPart {
            only_when: Some(Role::Nested),
            ..Default::default()
        };
        let unconditional_part = FormattedPart::default();

        let mut host_state = ZellijState::default();
        host_state.mode.session_ancestry = vec![];

        let mut nested_state = ZellijState::default();
        nested_state.mode.session_ancestry = vec!["outer".to_owned()];

        assert!(host_part.role_matches(&host_state));
        assert!(!host_part.role_matches(&nested_state));

        assert!(!nested_part.role_matches(&host_state));
        assert!(nested_part.role_matches(&nested_state));

        assert!(unconditional_part.role_matches(&host_state));
        assert!(unconditional_part.role_matches(&nested_state));
    }

    #[test]
    fn test_role_matches_treats_fullscreen_nested_session_as_host() {
        let host_part = FormattedPart {
            only_when: Some(Role::Host),
            ..Default::default()
        };
        let nested_part = FormattedPart {
            only_when: Some(Role::Nested),
            ..Default::default()
        };

        let mut fullscreen_nested_state = ZellijState::default();
        fullscreen_nested_state.mode.session_ancestry = vec!["outer".to_owned()];
        fullscreen_nested_state.mode.host_fullscreen = Some(true);

        assert!(host_part.role_matches(&fullscreen_nested_state));
        assert!(!nested_part.role_matches(&fullscreen_nested_state));
    }
}
