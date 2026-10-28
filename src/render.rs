use cached::{LruCache, macros::cached};
use lazy_static::lazy_static;
use std::{collections::BTreeMap, sync::Arc};

use anstyle::{Ansi256Color, AnsiColor, Color, RgbColor, Style};
use regex::Regex;
use zellij_tile::prelude::{PaletteColor, StyleDeclaration, Styling, bail};

use crate::{
    config::{UpdateEventMask, ZellijState, event_mask_from_widget_name},
    widgets::widget::Widget,
};

lazy_static! {
    static ref WIDGET_REGEX: Regex = Regex::new("(\\{[a-z_0-9]+\\})").unwrap();
}

/// Represents a color that may need runtime resolution
#[derive(Clone, Debug, PartialEq)]
pub enum ColorSource {
    /// Static color resolved at parse time (hex, named ANSI colors, 256-color palette)
    Static(Color),
    /// Zellij palette color resolved at render time from mode.style.colors
    /// Examples: "text_selected.base", "ribbon_unselected.emphasis_1"
    Zellij(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormattedPart {
    pub fg: Option<ColorSource>,
    pub bg: Option<ColorSource>,
    pub us: Option<ColorSource>,
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

            if part.eq("reverse") {
                result.reverse = true;
            }

            result.parse_and_set_effect(part);
        }

        // ModeUpdate carries theme changes, so invalidate cached formatting
        // that depends on the Zellij palette.
        let has_zellij_colors = [&result.fg, &result.bg, &result.us]
            .iter()
            .any(|color_opt| matches!(color_opt, Some(ColorSource::Zellij(_))));

        if has_zellij_colors {
            result.cache_mask |= UpdateEventMask::Mode as u8;
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

    pub fn format_string(&self, text: &str, state: &ZellijState) -> String {
        let mut style = Style::new();

        style = style.fg_color(resolve_color_source(self.fg.as_ref(), state));
        style = style.bg_color(resolve_color_source(self.bg.as_ref(), state));
        style = style.underline_color(resolve_color_source(self.us.as_ref(), state));
        style = style.effects(self.effects);

        format!(
            "{}{}{}{}",
            style.render_reset(),
            style.render(),
            text,
            style.render_reset()
        )
    }

    #[tracing::instrument(skip_all)]
    pub fn format_string_with_widgets(
        &mut self,
        widgets: &BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
    ) -> String {
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

        let res = self.format_string(&output, state);
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

fn hex_to_rgb(s: &str) -> anyhow::Result<Vec<u8>> {
    if s.len() != 6 {
        bail!("wrong hex color length");
    }

    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(anyhow::Error::from))
        .collect()
}

/// Looks up the [`StyleDeclaration`] for a Zellij style category name. Returns:
/// - `Some(Some(&decl))` when the category is recognized and present in the palette.
/// - `Some(None)` for `frame_unselected` when the palette omits it (Zellij treats this
///   as "no frame for unselected panes"; resolving a color against it yields `None`).
/// - `None` when the category is not a recognized Zellij category at all.
fn style_declaration_for_category<'a>(
    styling: &'a Styling,
    category: &str,
) -> Option<Option<&'a StyleDeclaration>> {
    Some(Some(match category {
        "text_unselected" => &styling.text_unselected,
        "text_selected" => &styling.text_selected,
        "ribbon_unselected" => &styling.ribbon_unselected,
        "ribbon_selected" => &styling.ribbon_selected,
        "table_title" => &styling.table_title,
        "table_cell_unselected" => &styling.table_cell_unselected,
        "table_cell_selected" => &styling.table_cell_selected,
        "list_unselected" => &styling.list_unselected,
        "list_selected" => &styling.list_selected,
        "frame_unselected" => return Some(styling.frame_unselected.as_ref()),
        "frame_selected" => &styling.frame_selected,
        "frame_highlight" => &styling.frame_highlight,
        "exit_code_success" => &styling.exit_code_success,
        "exit_code_error" => &styling.exit_code_error,
        _ => return None,
    }))
}

/// Checks for `category.field` references to Zellij's [`StyleDeclaration`] colors.
fn is_zellij_color_name(name: &str) -> bool {
    let Some((category, field)) = name.split_once('.') else {
        return false;
    };
    style_declaration_for_category(&Styling::default(), category).is_some()
        && get_style_field(&StyleDeclaration::default(), field).is_some()
}

#[cached(
    ty = "LruCache<String, ColorSource>",
    create = "{ LruCache::builder().max_size(100).build().unwrap() }",
    convert = r#"{ (color.to_owned()) }"#
)]
fn parse_color(color: &str, config: &BTreeMap<String, String>) -> Option<ColorSource> {
    let mut color = color;
    if color.starts_with('$') {
        let alias_name = color.strip_prefix('$').unwrap();

        color = config.get(&format!("color_{alias_name}"))?;
    }

    // Check if this looks like a Zellij color name - keep dynamic for runtime resolution
    // Examples: "text_selected.base", "ribbon_unselected.emphasis_1".
    if is_zellij_color_name(color) {
        return Some(ColorSource::Zellij(color.to_owned()));
    }

    if color.starts_with('#') {
        let rgb = match hex_to_rgb(color.strip_prefix('#').unwrap()) {
            Ok(rgb) => rgb,
            Err(_) => return None,
        };

        if rgb.len() != 3 {
            return None;
        }

        return Some(ColorSource::Static(
            RgbColor(
                *rgb.first().unwrap(),
                *rgb.get(1).unwrap(),
                *rgb.get(2).unwrap(),
            )
            .into(),
        ));
    }

    if let Some(color) = color_by_name(color) {
        return Some(ColorSource::Static(color.into()));
    }

    if color.starts_with("colour") {
        color = color.strip_prefix("colour").unwrap();
    }

    if let Ok(result) = color.parse::<u8>() {
        return Some(ColorSource::Static(Ansi256Color(result).into()));
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

/// Resolves a ColorSource to an actual Color at render time
fn resolve_color_source(source: Option<&ColorSource>, state: &ZellijState) -> Option<Color> {
    match source? {
        ColorSource::Static(color) => Some(*color),
        ColorSource::Zellij(name) => resolve_zellij_color(name, &state.mode.style.colors),
    }
}

/// Resolves a Zellij color name against the current palette. Returns `None` when the
/// name is unrecognized or the referenced palette slot is absent (e.g. `frame_unselected`
/// when the theme omits it); the caller then leaves the corresponding fg/bg/us unset.
fn resolve_zellij_color(name: &str, styling: &Styling) -> Option<Color> {
    let (category, field) = name.split_once('.')?;
    let style_decl = style_declaration_for_category(styling, category)??;
    let palette_color = get_style_field(style_decl, field)?;
    Some(palette_color_to_anstyle(palette_color))
}

/// Extracts a specific field from a StyleDeclaration
fn get_style_field(style: &StyleDeclaration, field: &str) -> Option<PaletteColor> {
    match field {
        "base" => Some(style.base),
        "background" => Some(style.background),
        "emphasis_0" => Some(style.emphasis_0),
        "emphasis_1" => Some(style.emphasis_1),
        "emphasis_2" => Some(style.emphasis_2),
        "emphasis_3" => Some(style.emphasis_3),
        _ => None,
    }
}

/// Converts a Zellij PaletteColor to an anstyle Color
fn palette_color_to_anstyle(palette_color: PaletteColor) -> Color {
    match palette_color {
        PaletteColor::Rgb((r, g, b)) => RgbColor(r, g, b).into(),
        PaletteColor::EightBit(n) => Ansi256Color(n).into(),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::widgets::{session::SessionWidget, tabs::TabsWidget};
    use zellij_tile::prelude::TabInfo;

    #[test]
    fn test_theme_colors_refresh_cached_format() {
        let config = BTreeMap::from([(
            "color_format_accent".to_owned(),
            "frame_selected.base".to_owned(),
        )]);
        let mut part = FormattedPart::from_format_string(
            "#[fg=$format_accent,bg=text_unselected.background,us=text_selected.emphasis_0]{session}",
            &config,
        );
        let widgets: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "session".to_owned(),
            Arc::new(SessionWidget::new(&config)) as Arc<dyn Widget>,
        )]);
        let mut state = ZellijState::default();
        state.mode.session_name = Some("test".to_owned());
        state.mode.style.colors.frame_selected.base = PaletteColor::Rgb((1, 2, 3));
        state.mode.style.colors.text_unselected.background = PaletteColor::EightBit(4);
        state.mode.style.colors.text_selected.emphasis_0 = PaletteColor::EightBit(5);

        let initial = part.format_string_with_widgets(&widgets, &state);
        assert!(initial.contains("\x1b[38;2;1;2;3m"));
        assert!(initial.contains("\x1b[48;5;4m"));
        assert!(initial.contains("\x1b[58;5;5m"));

        state.mode.style.colors.frame_selected.base = PaletteColor::EightBit(6);
        state.mode.style.colors.text_unselected.background = PaletteColor::Rgb((7, 8, 9));
        state.mode.style.colors.text_selected.emphasis_0 = PaletteColor::EightBit(10);
        // Without an update, the previously rendered format is cached.
        assert_eq!(part.format_string_with_widgets(&widgets, &state), initial);

        state.cache_mask = UpdateEventMask::Mode as u8;
        let updated = part.format_string_with_widgets(&widgets, &state);
        assert!(updated.contains("\x1b[38;5;6m"));
        assert!(updated.contains("\x1b[48;2;7;8;9m"));
        assert!(updated.contains("\x1b[58;5;10m"));
        assert!(updated.contains("test"));
    }

    #[test]
    fn test_theme_colors_refresh_cached_widget() {
        let config = BTreeMap::from([
            (
                "color_tab_accent".to_owned(),
                "frame_selected.base".to_owned(),
            ),
            (
                "tab_active".to_owned(),
                "#[fg=$tab_accent]{name}".to_owned(),
            ),
        ]);
        let mut part = FormattedPart::from_format_string("{tabs}", &config);
        let widgets: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "tabs".to_owned(),
            Arc::new(TabsWidget::new(&config)) as Arc<dyn Widget>,
        )]);
        let mut state = ZellijState {
            tabs: vec![TabInfo {
                name: "test".to_owned(),
                active: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        state.mode.style.colors.frame_selected.base = PaletteColor::EightBit(1);

        let initial = part.format_string_with_widgets(&widgets, &state);
        assert!(initial.contains("\x1b[38;5;1m"));

        state.mode.style.colors.frame_selected.base = PaletteColor::EightBit(5);
        assert_eq!(part.format_string_with_widgets(&widgets, &state), initial);

        state.cache_mask = UpdateEventMask::Mode as u8;
        let updated = part.format_string_with_widgets(&widgets, &state);
        assert!(updated.contains("\x1b[38;5;5m"));
        assert!(!updated.contains("\x1b[38;5;1m"));
        assert!(updated.contains("test"));
    }

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
        assert_eq!(result, Some(ColorSource::Static(RgbColor(1, 2, 3).into())));

        let result = parse_color("255", &config);
        assert_eq!(result, Some(ColorSource::Static(Ansi256Color(255).into())));

        let result = parse_color("365", &config);
        assert_eq!(result, None);

        let result = parse_color("#365", &config);
        assert_eq!(result, None);

        let result = parse_color("$green", &config);
        assert_eq!(
            result,
            Some(ColorSource::Static(RgbColor(0, 255, 0).into()))
        );

        let result = parse_color("$blue", &config);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_color_zellij_names() {
        let config: BTreeMap<String, String> = BTreeMap::new();

        assert_eq!(
            parse_color("text_selected.base", &config),
            Some(ColorSource::Zellij("text_selected.base".to_owned()))
        );
        assert_eq!(
            parse_color("ribbon_unselected.emphasis_1", &config),
            Some(ColorSource::Zellij(
                "ribbon_unselected.emphasis_1".to_owned()
            ))
        );
        assert_eq!(
            parse_color("frame_highlight.emphasis_3", &config),
            Some(ColorSource::Zellij("frame_highlight.emphasis_3".to_owned()))
        );
        assert_eq!(
            parse_color("exit_code_error.background", &config),
            Some(ColorSource::Zellij("exit_code_error.background".to_owned()))
        );
        assert_eq!(parse_color("player_1", &config), None);
        assert_eq!(parse_color("not_a_category.base", &config), None);
        assert_eq!(parse_color("text_selected.not_a_field", &config), None);
        assert_eq!(parse_color("text_selected", &config), None);
    }

    #[test]
    fn test_resolve_zellij_color_style_declaration() {
        let mut styling = Styling::default();
        styling.text_selected.base = PaletteColor::Rgb((1, 2, 3));
        styling.ribbon_unselected.emphasis_1 = PaletteColor::EightBit(7);

        assert_eq!(
            resolve_zellij_color("text_selected.base", &styling),
            Some(RgbColor(1, 2, 3).into())
        );
        assert_eq!(
            resolve_zellij_color("ribbon_unselected.emphasis_1", &styling),
            Some(Ansi256Color(7).into())
        );
        assert_eq!(resolve_zellij_color("bogus.base", &styling), None);
    }

    #[test]
    fn test_optional_theme_color_appears_and_disappears() {
        let part =
            FormattedPart::from_format_string("#[fg=frame_unselected.base]test", &BTreeMap::new());
        let mut state = ZellijState::default();
        state.mode.style.colors.frame_unselected = None;
        let unstyled = part.format_string("test", &state);
        assert_eq!(unstyled, "test");

        state.mode.style.colors.frame_unselected = Some(StyleDeclaration {
            base: PaletteColor::Rgb((5, 6, 7)),
            ..Default::default()
        });
        assert!(
            part.format_string("test", &state)
                .contains("\x1b[38;2;5;6;7m")
        );

        state.mode.style.colors.frame_unselected = None;
        assert_eq!(part.format_string("test", &state), unstyled);
    }
}
