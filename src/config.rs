use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use itertools::Itertools;
use regex::Regex;
use zellij_tile::prelude::*;

use crate::{
    border::{BorderConfig, BorderPosition, parse_border_config},
    render::FormattedPart,
    widgets::{command::CommandResult, notification, widget::Widget},
};
use chrono::{DateTime, Local};

/// Which sessions `dim_when_unfocused` applies to. Parsed from the
/// `dim_scope` config option.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimScope {
    /// Dim both a host that has descended into a nested child and a nested
    /// session not currently ascended into.
    #[default]
    All,
    /// Dim only a nested session not currently ascended into; leave a
    /// descended host's own chrome at full brightness.
    NestedOnly,
}

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
    pub focused_pane_id: Option<PaneId>,
    pub focused_pane_cwd: Option<std::path::PathBuf>,
    /// Config-time toggle for nested-session dimming (`dim_when_unfocused`,
    /// default `true`). Lives on state, not just `ModuleConfig`, so that
    /// every render-time call site that already has a `&ZellijState` (most
    /// of them, since it's the shared render context) can call
    /// `dim_amount()` without also needing the module config threaded in.
    pub dim_when_unfocused: bool,
    /// How strongly to dim, in `render::dim_color`'s `0.0..=1.0` scale.
    /// Parsed from the `dim_strength` config option.
    pub dim_strength: f32,
    /// Which sessions dimming applies to. Parsed from the `dim_scope`
    /// config option.
    pub dim_scope: DimScope,
}

impl ZellijState {
    /// The dim strength to render with right now: `0.0` (no change) unless
    /// `dim_when_unfocused` is enabled, this session is currently the
    /// dimmed side of a nested-session pair (a host that has descended
    /// into a child, or a nested session not currently ascended into),
    /// and `dim_scope` includes it. `session_ascended`/`session_dimmed`
    /// are the same fields core's own bundled tab-bar/compact-bar plugins
    /// use for this exact purpose.
    pub fn dim_amount(&self) -> f32 {
        let is_dimmed =
            self.mode.session_ascended == Some(true) || self.mode.session_dimmed == Some(true);
        let in_scope = match self.dim_scope {
            DimScope::All => true,
            // `session_ancestry` is only non-empty for a nested session,
            // so this excludes a host that has merely descended.
            DimScope::NestedOnly => !self.mode.session_ancestry.is_empty(),
        };

        if self.dim_when_unfocused && is_dimmed && in_scope {
            self.dim_strength
        } else {
            0.0
        }
    }
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
    pub pane_frame_style: String,
    pub border: BorderConfig,
    pub format_precedence: Vec<Part>,
    pub hide_on_overlength: bool,
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
        let pane_frame_style = match config.get("pane_frame_style") {
            Some(style) => style.to_owned(),
            None => "titles".to_owned(),
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
            pane_frame_style,
            border: border_config,
            format_precedence,
            hide_on_overlength,
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
            Mouse::ScrollLeft(_) => return,
            Mouse::ScrollRight(_) => return,
            Mouse::LeftClick(_, y) => y,
            Mouse::RightClick(_, y) => y,
            Mouse::Hold(_, y) => y,
            Mouse::Release(_, y) => y,
            Mouse::Hover(_, _) => return,
        };
        let dim = state.dim_amount();

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
                dim,
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
                dim,
            ));
        } else {
            offset += console::measure_text_width(&self.get_spacer(
                &output_left,
                &output_right,
                state.cols,
                dim,
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

        let dim = state.dim_amount();

        if self.border.enabled {
            let mut border_top = "".to_owned();
            if self.border.enabled && self.border.position == BorderPosition::Top {
                border_top = format!("{}\n", self.border.draw(state.cols, dim));
            }

            let mut border_bottom = "".to_owned();
            if self.border.enabled && self.border.position == BorderPosition::Bottom {
                border_bottom = format!("\n{}", self.border.draw(state.cols, dim));
            }

            if !output_center.is_empty() {
                return format!(
                    "{}{}{}{}{}{}{}",
                    border_top,
                    output_left,
                    self.get_spacer_left(&output_left, &output_center, state.cols, dim),
                    output_center,
                    self.get_spacer_right(&output_right, &output_center, state.cols, dim),
                    output_right,
                    border_bottom,
                );
            }

            return format!(
                "{}{}{}{}{}",
                border_top,
                output_left,
                self.get_spacer(&output_left, &output_right, state.cols, dim),
                output_right,
                border_bottom,
            );
        }

        if !output_center.is_empty() {
            return format!(
                "{}{}{}{}{}",
                output_left,
                self.get_spacer_left(&output_left, &output_center, state.cols, dim),
                output_center,
                self.get_spacer_right(&output_right, &output_center, state.cols, dim),
                output_right,
            );
        }

        format!(
            "{}{}{}",
            output_left,
            self.get_spacer(&output_left, &output_right, state.cols, dim),
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
    fn get_spacer_left(
        &self,
        output_left: &str,
        output_center: &str,
        cols: usize,
        dim: f32,
    ) -> String {
        let text_count = console::measure_text_width(output_left)
            + (console::measure_text_width(output_center) as f32 / 2.0).floor() as usize;

        let center_pos = (cols as f32 / 2.0).floor() as usize;

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = center_pos.saturating_sub(text_count);

        tracing::debug!("space_count: {:?}", space_count);
        self.format_space
            .format_string(&" ".repeat(space_count), dim)
    }

    #[tracing::instrument(skip_all)]
    fn get_spacer_right(
        &self,
        output_right: &str,
        output_center: &str,
        cols: usize,
        dim: f32,
    ) -> String {
        let text_count = console::measure_text_width(output_right)
            + (console::measure_text_width(output_center) as f32 / 2.0).ceil() as usize;

        let center_pos = (cols as f32 / 2.0).ceil() as usize;

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = center_pos.saturating_sub(text_count);

        tracing::debug!("space_count: {:?}", space_count);
        self.format_space
            .format_string(&" ".repeat(space_count), dim)
    }

    fn get_spacer(&self, output_left: &str, output_right: &str, cols: usize, dim: f32) -> String {
        let text_count =
            console::measure_text_width(output_left) + console::measure_text_width(output_right);

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = cols.saturating_sub(text_count);

        self.format_space
            .format_string(&" ".repeat(space_count), dim)
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

    fn state_with(
        dim_when_unfocused: bool,
        dim_strength: f32,
        dim_scope: DimScope,
        session_ancestry: Vec<String>,
        session_ascended: Option<bool>,
        session_dimmed: Option<bool>,
    ) -> ZellijState {
        ZellijState {
            mode: ModeInfo {
                session_ascended,
                session_dimmed,
                session_ancestry,
                ..Default::default()
            },
            dim_when_unfocused,
            dim_strength,
            dim_scope,
            ..Default::default()
        }
    }

    #[test]
    fn test_dim_amount_when_not_dimmed() {
        let state = state_with(true, 0.5, DimScope::All, vec![], Some(false), None);
        assert_eq!(state.dim_amount(), 0.0);
    }

    #[test]
    fn test_dim_amount_when_session_ascended() {
        let state = state_with(
            true,
            0.5,
            DimScope::All,
            vec!["main".to_owned()],
            Some(true),
            None,
        );
        assert_eq!(state.dim_amount(), 0.5);
    }

    #[test]
    fn test_dim_amount_when_session_dimmed() {
        let state = state_with(true, 0.7, DimScope::All, vec![], None, Some(true));
        assert_eq!(state.dim_amount(), 0.7);
    }

    #[test]
    fn test_dim_amount_respects_config_toggle() {
        // Dimmed by the session, but the user has turned the feature off.
        let state = state_with(
            false,
            0.5,
            DimScope::All,
            vec!["main".to_owned()],
            Some(true),
            Some(true),
        );
        assert_eq!(state.dim_amount(), 0.0);
    }

    #[test]
    fn test_dim_amount_nested_only_dims_nested_session() {
        let state = state_with(
            true,
            0.5,
            DimScope::NestedOnly,
            vec!["main".to_owned()],
            Some(true),
            None,
        );
        assert_eq!(state.dim_amount(), 0.5);
    }

    #[test]
    fn test_dim_amount_nested_only_ignores_descended_host() {
        // A host that has descended has no ancestry of its own, so
        // NestedOnly should leave its chrome at full brightness.
        let state = state_with(true, 0.5, DimScope::NestedOnly, vec![], None, Some(true));
        assert_eq!(state.dim_amount(), 0.0);
    }
}
