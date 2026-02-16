use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use itertools::Itertools;
use regex::Regex;
use zellij_tile::prelude::*;

use crate::{
    border::{parse_border_config, BorderConfig, BorderPosition},
    render::{FormattedPart, color_to_format_string},
    widgets::{command::CommandResult, notification, widget::Widget},
};
use chrono::{DateTime, Local};

#[derive(Default, Debug, Clone)]
pub struct ZellijState {
    pub cols: usize,
    pub command_results: BTreeMap<String, CommandResult>,
    pub pipe_results: BTreeMap<String, String>,
    pub mode: ModeInfo,
    pub panes: PaneManifest,
    pub plugin_uuid: String,
    pub tabs: Vec<TabInfo>,
    pub sessions: Vec<SessionInfo>,
    pub start_time: DateTime<Local>,
    pub incoming_notification: Option<notification::Message>,
    pub cache_mask: u8,
}

#[derive(Clone, Debug, Ord, Eq, PartialEq, PartialOrd, Copy)]
pub enum Part {
    Left,
    Center,
    Right,
}

impl FromStr for Part {
    fn from_str(part: &str) -> Result<Self> {
        match part {
            "l" => Ok(Part::Left),
            "c" => Ok(Part::Center),
            "r" => Ok(Part::Right),
            _ => anyhow::bail!("Invalid part: {}", part),
        }
    }

    type Err = anyhow::Error;
}

pub enum UpdateEventMask {
    Always = 0b10000000,
    Mode = 0b00000001,
    Tab = 0b00000011,
    Command = 0b00000100,
    Session = 0b00001000,
    None = 0b00000000,
}

pub fn event_mask_from_widget_name(name: &str) -> u8 {
    match name {
        "command" => UpdateEventMask::Always as u8,
        "datetime" => UpdateEventMask::Always as u8,
        "mode" => UpdateEventMask::Mode as u8,
        "notifications" => UpdateEventMask::Always as u8,
        "session" => UpdateEventMask::Mode as u8,
        "swap_layout" => UpdateEventMask::Tab as u8,
        "tabs" => UpdateEventMask::Tab as u8,
        "pipe" => UpdateEventMask::Always as u8,
        _ => UpdateEventMask::None as u8,
    }
}

#[derive(Default, Debug)]
pub struct ModuleConfig {
    pub left_parts_config: String,
    pub left_parts: Vec<FormattedPart>,
    pub center_parts_config: String,
    pub center_parts: Vec<FormattedPart>,
    pub right_parts_config: String,
    pub right_parts: Vec<FormattedPart>,
    pub format_space: FormattedPart,
    pub hide_frame_for_single_pane: bool,
    pub hide_frame_except_for_search: bool,
    pub hide_frame_except_for_fullscreen: bool,
    pub hide_frame_except_for_scroll: bool,
    pub border: BorderConfig,
    pub format_precedence: Vec<Part>,
    pub hide_on_overlength: bool,
    uses_mode_colors: bool,
    plugin_config: BTreeMap<String, String>,
    last_mode: Option<InputMode>,
}

impl ModuleConfig {
    pub fn new(config: &BTreeMap<String, String>) -> anyhow::Result<Self> {
        let format_space_config = match config.get("format_space") {
            Some(space_config) => space_config,
            None => "",
        };

        let hide_frame_for_single_pane = match config.get("hide_frame_for_single_pane") {
            Some(toggle) => toggle == "true",
            None => false,
        };
        let hide_frame_except_for_search = match config.get("hide_frame_except_for_search") {
            Some(toggle) => toggle == "true",
            None => false,
        };
        let hide_frame_except_for_fullscreen = match config.get("hide_frame_except_for_fullscreen")
        {
            Some(toggle) => toggle == "true",
            None => false,
        };
        let hide_frame_except_for_scroll = match config.get("hide_frame_except_for_scroll") {
            Some(toggle) => toggle == "true",
            None => false,
        };

        let left_parts_config = match config.get("format_left") {
            Some(conf) => conf,
            None => "",
        };

        let right_parts_config = match config.get("format_right") {
            Some(conf) => conf,
            None => "",
        };

        let center_parts_config = match config.get("format_center") {
            Some(conf) => conf,
            None => "",
        };

        let format_precedence = match config.get("format_precedence") {
            Some(conf) => {
                let prec = conf
                    .chars()
                    .map(|c| Part::from_str(&c.to_string()))
                    .collect();

                match prec {
                    Ok(prec) => prec,
                    Err(e) => {
                        anyhow::bail!("Invalid format_precedence: {}", e);
                    }
                }
            }
            None => vec![Part::Left, Part::Center, Part::Right],
        };

        let hide_on_overlength = match config.get("format_hide_on_overlength") {
            Some(opt) => opt == "true",
            None => false,
        };

        let border_config = parse_border_config(config).unwrap_or_default();

        let uses_mode_colors = [left_parts_config, center_parts_config, right_parts_config]
            .iter()
            .any(|s| s.contains("{mode_bg}") || s.contains("{mode_fg}"));

        Ok(Self {
            left_parts_config: left_parts_config.to_owned(),
            left_parts: parts_from_config(Some(&left_parts_config.to_owned()), config),
            center_parts_config: center_parts_config.to_owned(),
            center_parts: parts_from_config(Some(&center_parts_config.to_owned()), config),
            right_parts_config: right_parts_config.to_owned(),
            right_parts: parts_from_config(Some(&right_parts_config.to_owned()), config),
            format_space: FormattedPart::from_format_string(format_space_config, config),
            hide_frame_for_single_pane,
            hide_frame_except_for_search,
            hide_frame_except_for_fullscreen,
            hide_frame_except_for_scroll,
            border: border_config,
            format_precedence,
            hide_on_overlength,
            uses_mode_colors,
            plugin_config: config.clone(),
            last_mode: None,
        })
    }

    pub fn handle_mouse_action(
        &mut self,
        state: ZellijState,
        mouse: Mouse,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
    ) {
        let click_pos = match mouse {
            Mouse::ScrollUp(_) => return,
            Mouse::ScrollDown(_) => return,
            Mouse::LeftClick(_, y) => y,
            Mouse::RightClick(_, y) => y,
            Mouse::Hold(_, y) => y,
            Mouse::Release(_, y) => y,
            Mouse::Hover(_, _) => return,
        };

        self.resolve_mode_colors(state.mode.mode);

        let output_left = self.left_parts.iter_mut().fold("".to_owned(), |acc, part| {
            format!(
                "{}{}",
                acc,
                part.format_string_with_widgets(&widget_map, &state)
            )
        });

        let output_center = self
            .center_parts
            .iter_mut()
            .fold("".to_owned(), |acc, part| {
                format!(
                    "{}{}",
                    acc,
                    part.format_string_with_widgets(&widget_map, &state)
                )
            });

        let output_right = self
            .right_parts
            .iter_mut()
            .fold("".to_owned(), |acc, part| {
                format!(
                    "{}{}",
                    acc,
                    part.format_string_with_widgets(&widget_map, &state)
                )
            });

        let (output_left, output_center, output_right) = match self.hide_on_overlength {
            true => self.trim_output(&output_left, &output_center, &output_right, state.cols),
            false => (output_left, output_center, output_right),
        };

        let mut offset = console::measure_text_width(&output_left);

        self.process_widget_click(click_pos, &self.left_parts, &widget_map, &state, 0);

        if click_pos <= offset {
            return;
        }

        if !output_center.is_empty() {
            tracing::debug!("widgetclick center");
            offset += console::measure_text_width(&self.get_spacer_left(
                &output_left,
                &output_center,
                state.cols,
            ));

            offset += self.process_widget_click(
                click_pos,
                &self.center_parts,
                &widget_map,
                &state,
                offset,
            );

            if click_pos <= offset {
                return;
            }

            offset += console::measure_text_width(&self.get_spacer_right(
                &output_right,
                &output_center,
                state.cols,
            ));
        } else {
            offset += console::measure_text_width(&self.get_spacer(
                &output_left,
                &output_right,
                state.cols,
            ));
        }

        self.process_widget_click(click_pos, &self.right_parts, &widget_map, &state, offset);
    }

    fn process_widget_click(
        &self,
        click_pos: usize,
        widgets: &[FormattedPart],
        widget_map: &BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
        offset: usize,
    ) -> usize {
        let widget_string = widgets.iter().fold(String::new(), |a, b| a + &b.content);

        let mut rendered_output = widget_string.clone();

        let tokens: Vec<String> = widget_map.keys().map(|k| k.to_owned()).collect();

        let widgets_regex = Regex::new("(\\{[a-z_0-9]+\\})").unwrap();
        for widget in widgets_regex.captures_iter(widget_string.as_str()) {
            let match_name = widget.get(0).unwrap().as_str();
            let widget_key = match_name.trim_matches(|c| c == '{' || c == '}');
            let mut widget_key_name = widget_key;

            if widget_key.starts_with("command_") {
                widget_key_name = "command";
            }

            if widget_key.starts_with("pipe_") {
                widget_key_name = "pipe";
            }

            if !tokens.contains(&widget_key_name.to_owned()) {
                continue;
            }

            let wid = match widget_map.get(widget_key_name) {
                Some(wid) => wid,
                None => continue,
            };

            let pos = match rendered_output.find(match_name) {
                Some(_pos) => {
                    let pref = rendered_output.split(match_name).collect::<Vec<&str>>()[0];
                    console::measure_text_width(pref)
                }
                None => continue,
            };

            let wid_res = wid.process(widget_key, state);
            rendered_output = rendered_output.replace(match_name, &wid_res);

            if click_pos < pos + offset
                || click_pos > pos + offset + console::measure_text_width(&wid_res)
            {
                continue;
            }

            wid.process_click(widget_key, state, click_pos - (pos + offset));
        }

        console::measure_text_width(&rendered_output)
    }

    pub fn render_bar(
        &mut self,
        state: ZellijState,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
    ) -> String {
        if self.left_parts.is_empty() && self.center_parts.is_empty() && self.right_parts.is_empty()
        {
            return "No configuration found. See https://github.com/dj95/zjstatus/wiki/3-%E2%80%90-Configuration for more info".to_string();
        }

        self.resolve_mode_colors(state.mode.mode);

        let output_left = self.left_parts.iter_mut().fold("".to_owned(), |acc, part| {
            format!(
                "{acc}{}",
                part.format_string_with_widgets(&widget_map, &state)
            )
        });

        let output_center = self
            .center_parts
            .iter_mut()
            .fold("".to_owned(), |acc, part| {
                format!(
                    "{acc}{}",
                    part.format_string_with_widgets(&widget_map, &state)
                )
            });

        let output_right = self
            .right_parts
            .iter_mut()
            .fold("".to_owned(), |acc, part| {
                format!(
                    "{acc}{}",
                    part.format_string_with_widgets(&widget_map, &state)
                )
            });

        let (output_left, output_center, output_right) = match self.hide_on_overlength {
            true => self.trim_output(&output_left, &output_center, &output_right, state.cols),
            false => (output_left, output_center, output_right),
        };

        if self.border.enabled {
            let mut border_top = "".to_owned();
            if self.border.enabled && self.border.position == BorderPosition::Top {
                border_top = format!("{}\n", self.border.draw(state.cols));
            }

            let mut border_bottom = "".to_owned();
            if self.border.enabled && self.border.position == BorderPosition::Bottom {
                border_bottom = format!("\n{}", self.border.draw(state.cols));
            }

            if !output_center.is_empty() {
                return format!(
                    "{}{}{}{}{}{}{}",
                    border_top,
                    output_left,
                    self.get_spacer_left(&output_left, &output_center, state.cols),
                    output_center,
                    self.get_spacer_right(&output_right, &output_center, state.cols),
                    output_right,
                    border_bottom,
                );
            }

            return format!(
                "{}{}{}{}{}",
                border_top,
                output_left,
                self.get_spacer(&output_left, &output_right, state.cols),
                output_right,
                border_bottom,
            );
        }

        if !output_center.is_empty() {
            return format!(
                "{}{}{}{}{}",
                output_left,
                self.get_spacer_left(&output_left, &output_center, state.cols),
                output_center,
                self.get_spacer_right(&output_right, &output_center, state.cols),
                output_right,
            );
        }

        format!(
            "{}{}{}",
            output_left,
            self.get_spacer(&output_left, &output_right, state.cols),
            output_right,
        )
    }

    fn trim_output(
        &self,
        output_left: &str,
        output_center: &str,
        output_right: &str,
        cols: usize,
    ) -> (String, String, String) {
        let center_pos = (cols as f32 / 2.0).floor() as usize;

        let mut output = BTreeMap::from([
            (Part::Left, output_left.to_owned()),
            (Part::Center, output_center.to_owned()),
            (Part::Right, output_right.to_owned()),
        ]);

        let combinations = [
            (self.format_precedence[2], self.format_precedence[1]),
            (self.format_precedence[1], self.format_precedence[0]),
            (self.format_precedence[2], self.format_precedence[0]),
        ];

        for win in combinations.iter() {
            let (a, b) = win;

            let part_a = output.get(a).unwrap();
            let part_b = output.get(b).unwrap();

            let a_count = console::measure_text_width(part_a);
            let b_count = console::measure_text_width(part_b);

            let overlap = match (a, b) {
                (Part::Left, Part::Right) => a_count + b_count > cols,
                (Part::Right, Part::Left) => a_count + b_count > cols,
                (Part::Left, Part::Center) => a_count > center_pos - (b_count / 2),
                (Part::Center, Part::Left) => b_count > center_pos - (a_count / 2),
                (Part::Right, Part::Center) => a_count > center_pos - (b_count / 2),
                (Part::Center, Part::Right) => b_count > center_pos - (a_count / 2),
                _ => false,
            };

            if overlap {
                output.insert(*a, "".to_owned());
            }
        }

        output.values().cloned().collect_tuple().unwrap()
    }

    #[tracing::instrument(skip_all)]
    fn get_spacer_left(&self, output_left: &str, output_center: &str, cols: usize) -> String {
        let text_count = console::measure_text_width(output_left)
            + (console::measure_text_width(output_center) as f32 / 2.0).floor() as usize;

        let center_pos = (cols as f32 / 2.0).floor() as usize;

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = center_pos.saturating_sub(text_count);

        tracing::debug!("space_count: {:?}", space_count);
        self.format_space.format_string(&" ".repeat(space_count))
    }

    #[tracing::instrument(skip_all)]
    fn get_spacer_right(&self, output_right: &str, output_center: &str, cols: usize) -> String {
        let text_count = console::measure_text_width(output_right)
            + (console::measure_text_width(output_center) as f32 / 2.0).ceil() as usize;

        let center_pos = (cols as f32 / 2.0).ceil() as usize;

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = center_pos.saturating_sub(text_count);

        tracing::debug!("space_count: {:?}", space_count);
        self.format_space.format_string(&" ".repeat(space_count))
    }

    fn get_spacer(&self, output_left: &str, output_right: &str, cols: usize) -> String {
        let text_count =
            console::measure_text_width(output_left) + console::measure_text_width(output_right);

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = cols.saturating_sub(text_count);

        self.format_space.format_string(&" ".repeat(space_count))
    }

    fn resolve_mode_colors(&mut self, mode: InputMode) {
        if !self.uses_mode_colors || self.last_mode == Some(mode) {
            return;
        }

        let (fg, bg) = get_mode_colors(&self.plugin_config, mode);
        let resolve =
            |s: &str| s.replace("{mode_fg}", &fg).replace("{mode_bg}", &bg);

        self.left_parts = parts_from_config(
            Some(&resolve(&self.left_parts_config)),
            &self.plugin_config,
        );
        self.center_parts = parts_from_config(
            Some(&resolve(&self.center_parts_config)),
            &self.plugin_config,
        );
        self.right_parts = parts_from_config(
            Some(&resolve(&self.right_parts_config)),
            &self.plugin_config,
        );
        self.last_mode = Some(mode);
    }
}

fn get_mode_colors(config: &BTreeMap<String, String>, mode: InputMode) -> (String, String) {
    let mode_key = match mode {
        InputMode::Normal => "mode_normal",
        InputMode::Locked => "mode_locked",
        InputMode::Resize => "mode_resize",
        InputMode::Pane => "mode_pane",
        InputMode::Tab => "mode_tab",
        InputMode::Scroll => "mode_scroll",
        InputMode::EnterSearch => "mode_enter_search",
        InputMode::Search => "mode_search",
        InputMode::RenameTab => "mode_rename_tab",
        InputMode::RenamePane => "mode_rename_pane",
        InputMode::Session => "mode_session",
        InputMode::Move => "mode_move",
        InputMode::Prompt => "mode_prompt",
        InputMode::Tmux => "mode_tmux",
    };

    let format_str = config
        .get(mode_key)
        .or_else(|| {
            config
                .get("mode_default_to_mode")
                .and_then(|default| config.get(&format!("mode_{}", default)))
        })
        .or_else(|| config.get("mode_normal"));

    match format_str {
        Some(s) => {
            let parts = FormattedPart::multiple_from_format_string(s, config);
            match parts.last() {
                Some(part) => (
                    color_to_format_string(part.fg),
                    color_to_format_string(part.bg),
                ),
                None => ("default".to_string(), "default".to_string()),
            }
        }
        None => ("default".to_string(), "default".to_string()),
    }
}

fn parts_from_config(
    format: Option<&String>,
    config: &BTreeMap<String, String>,
) -> Vec<FormattedPart> {
    match format {
        Some(format) => match format.is_empty() {
            true => vec![],
            false => format
                .split("#[")
                .map(|s| FormattedPart::from_format_string(s, config))
                .collect(),
        },
        None => vec![],
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anstyle::{Effects, RgbColor};

    #[test]
    fn test_formatted_part_from_string() {
        let input = "#[fg=#ff0000,bg=#00ff00,bold,italic]foo";

        let part = FormattedPart::from_format_string(input, &BTreeMap::new());

        assert_eq!(
            part,
            FormattedPart {
                fg: Some(RgbColor(255, 0, 0).into()),
                bg: Some(RgbColor(0, 255, 0).into()),
                effects: Effects::BOLD | Effects::ITALIC,
                content: "foo".to_owned(),
                ..Default::default()
            },
        )
    }

    #[test]
    fn test_get_mode_colors() {
        let mut config: BTreeMap<String, String> = BTreeMap::new();
        config.insert(
            "mode_locked".to_owned(),
            "#[bg=#31748f,fg=#191724,bold]  LOCKED ".to_owned(),
        );
        config.insert(
            "mode_tmux".to_owned(),
            "#[bg=#eb6f92,fg=#191724,bold]  COMMAND ".to_owned(),
        );

        let (fg, bg) = get_mode_colors(&config, InputMode::Locked);
        assert_eq!(fg, "#191724");
        assert_eq!(bg, "#31748f");

        let (fg, bg) = get_mode_colors(&config, InputMode::Tmux);
        assert_eq!(fg, "#191724");
        assert_eq!(bg, "#eb6f92");
    }

    #[test]
    fn test_get_mode_colors_with_default_to_mode() {
        let mut config: BTreeMap<String, String> = BTreeMap::new();
        config.insert(
            "mode_tmux".to_owned(),
            "#[bg=#eb6f92,fg=#191724]  COMMAND ".to_owned(),
        );
        config.insert("mode_default_to_mode".to_owned(), "tmux".to_owned());

        // Locked is not defined, should fall back to tmux via default_to_mode
        let (fg, bg) = get_mode_colors(&config, InputMode::Locked);
        assert_eq!(fg, "#191724");
        assert_eq!(bg, "#eb6f92");
    }

    #[test]
    fn test_get_mode_colors_fallback_to_normal() {
        let mut config: BTreeMap<String, String> = BTreeMap::new();
        config.insert(
            "mode_normal".to_owned(),
            "#[bg=#89b4fa,fg=#181825] ".to_owned(),
        );

        // Locked is not defined, no default_to_mode, should fall back to normal
        let (fg, bg) = get_mode_colors(&config, InputMode::Locked);
        assert_eq!(fg, "#181825");
        assert_eq!(bg, "#89b4fa");
    }

    #[test]
    fn test_get_mode_colors_none_defined() {
        let config: BTreeMap<String, String> = BTreeMap::new();

        let (fg, bg) = get_mode_colors(&config, InputMode::Normal);
        assert_eq!(fg, "default");
        assert_eq!(bg, "default");
    }

    #[test]
    fn test_uses_mode_colors_detection() {
        let mut config: BTreeMap<String, String> = BTreeMap::new();
        config.insert(
            "format_left".to_owned(),
            "{mode}#[bg={mode_bg},fg={mode_fg}] {session}".to_owned(),
        );
        config.insert("format_right".to_owned(), "".to_owned());
        config.insert("format_center".to_owned(), "".to_owned());

        let module = ModuleConfig::new(&config).unwrap();
        assert!(module.uses_mode_colors);
    }

    #[test]
    fn test_does_not_use_mode_colors() {
        let mut config: BTreeMap<String, String> = BTreeMap::new();
        config.insert(
            "format_left".to_owned(),
            "{mode}#[fg=#89B4FA] {session}".to_owned(),
        );

        let module = ModuleConfig::new(&config).unwrap();
        assert!(!module.uses_mode_colors);
    }

    #[test]
    fn test_resolve_mode_colors() {
        let mut config: BTreeMap<String, String> = BTreeMap::new();
        config.insert(
            "format_left".to_owned(),
            "#[bg={mode_bg},fg={mode_fg}] {session}".to_owned(),
        );
        config.insert(
            "mode_locked".to_owned(),
            "#[bg=#31748f,fg=#191724,bold]  LOCKED ".to_owned(),
        );
        config.insert(
            "mode_normal".to_owned(),
            "#[bg=#89b4fa,fg=#181825] ".to_owned(),
        );

        let mut module = ModuleConfig::new(&config).unwrap();
        assert!(module.uses_mode_colors);

        // Resolve for locked mode
        module.resolve_mode_colors(InputMode::Locked);
        assert_eq!(module.last_mode, Some(InputMode::Locked));
        // Format string "#[bg=...,fg=...] {session}" splits into 2 parts:
        // empty prefix + styled content
        assert_eq!(module.left_parts.len(), 2);
        assert_eq!(module.left_parts[1].bg, Some(RgbColor(0x31, 0x74, 0x8f).into()));
        assert_eq!(module.left_parts[1].fg, Some(RgbColor(0x19, 0x17, 0x24).into()));

        // Resolve for normal mode
        module.resolve_mode_colors(InputMode::Normal);
        assert_eq!(module.last_mode, Some(InputMode::Normal));
        assert_eq!(module.left_parts[1].bg, Some(RgbColor(0x89, 0xb4, 0xfa).into()));
        assert_eq!(module.left_parts[1].fg, Some(RgbColor(0x18, 0x18, 0x25).into()));
    }
}
