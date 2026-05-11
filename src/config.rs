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

#[derive(Default, Debug, Clone)]
pub struct ZellijState {
    pub cols: usize,
    pub command_results: BTreeMap<String, CommandResult>,
    pub pipe_results: BTreeMap<String, String>,
    pub pipe_scroll_offsets: BTreeMap<String, usize>,
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
}

#[derive(Clone, Debug, Ord, Eq, PartialEq, PartialOrd, Copy)]
pub enum Part {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
struct RenderedWidget {
    part: Part,
    widget_name: String,
    name: String,
    output: String,
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
    pub pipe_scroll_target: Option<String>,
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

        let pipe_scroll_target = config.get("pipe_scroll_target").cloned();

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
            border: border_config,
            format_precedence,
            hide_on_overlength,
            pipe_scroll_target,
        })
    }

    pub fn handle_mouse_action(
        &mut self,
        state: &mut ZellijState,
        mouse: Mouse,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
    ) -> bool {
        let click_pos = match mouse {
            Mouse::ScrollUp(lines) => {
                return self.process_widget_scroll(
                    &self.all_parts(),
                    &widget_map,
                    state,
                    -(lines.max(1) as isize),
                );
            }
            Mouse::ScrollDown(lines) => {
                return self.process_widget_scroll(
                    &self.all_parts(),
                    &widget_map,
                    state,
                    lines.max(1) as isize,
                );
            }
            Mouse::LeftClick(_, y) => y,
            Mouse::RightClick(_, y) => y,
            Mouse::Hold(_, y) => y,
            Mouse::Release(_, y) => y,
            Mouse::Hover(_, _) => return false,
        };

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

        self.process_widget_click(click_pos, &self.left_parts, &widget_map, state, 0);

        if click_pos <= offset {
            return false;
        }

        if !output_center.is_empty() {
            tracing::debug!("widgetclick center");
            offset += console::measure_text_width(&self.get_spacer_left(
                &output_left,
                &output_center,
                state.cols,
            ));

            let widget_width = self.process_widget_click(
                click_pos,
                &self.center_parts,
                &widget_map,
                state,
                offset,
            );
            offset += widget_width;

            if click_pos <= offset {
                return false;
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

        self.process_widget_click(click_pos, &self.right_parts, &widget_map, state, offset);
        false
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

    fn process_widget_scroll(
        &self,
        widgets: &[FormattedPart],
        widget_map: &BTreeMap<String, Arc<dyn Widget>>,
        state: &mut ZellijState,
        delta: isize,
    ) -> bool {
        for widget_key in Self::widget_keys(widgets) {
            if let Some(target) = &self.pipe_scroll_target
                && widget_key != *target
            {
                continue;
            }

            let mut widget_key_name = widget_key.as_str();

            if widget_key.starts_with("command_") {
                widget_key_name = "command";
            }

            if widget_key.starts_with("pipe_") {
                widget_key_name = "pipe";
            }

            let Some(widget) = widget_map.get(widget_key_name) else {
                continue;
            };

            if widget.process_scroll(&widget_key, state, delta) {
                return true;
            }
        }

        // TODO: document the fallback behavior: without pipe_scroll_target,
        // scroll events affect the first scrollable pipe in format order.
        false
    }

    fn all_parts(&self) -> Vec<FormattedPart> {
        self.left_parts
            .iter()
            .chain(self.center_parts.iter())
            .chain(self.right_parts.iter())
            .cloned()
            .collect()
    }

    fn widget_keys(widgets: &[FormattedPart]) -> Vec<String> {
        let widget_string = widgets.iter().fold(String::new(), |a, b| a + &b.content);
        let widgets_regex = Regex::new("(\\{[a-z_0-9]+\\})").unwrap();

        widgets_regex
            .captures_iter(widget_string.as_str())
            .map(|widget| {
                widget
                    .get(0)
                    .unwrap()
                    .as_str()
                    .trim_matches(|c| c == '{' || c == '}')
                    .to_owned()
            })
            .collect()
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

        let (mut output_left, mut rendered_widgets) =
            Self::render_parts(&mut self.left_parts, &widget_map, &state, Part::Left);

        let (mut output_center, center_widgets) =
            Self::render_parts(&mut self.center_parts, &widget_map, &state, Part::Center);
        rendered_widgets.extend(center_widgets);

        let (mut output_right, right_widgets) =
            Self::render_parts(&mut self.right_parts, &widget_map, &state, Part::Right);
        rendered_widgets.extend(right_widgets);

        self.truncate_outputs(
            &mut output_left,
            &mut output_center,
            &mut output_right,
            &rendered_widgets,
            &widget_map,
            &state,
            state.cols,
        );

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

    fn render_parts(
        parts: &mut [FormattedPart],
        widget_map: &BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
        part: Part,
    ) -> (String, Vec<RenderedWidget>) {
        let output = parts.iter_mut().fold("".to_owned(), |acc, format_part| {
            format!(
                "{acc}{}",
                format_part.format_string_with_widgets(widget_map, state)
            )
        });
        let widget_string = parts.iter().fold(String::new(), |a, b| a + &b.content);
        let mut rendered_widgets = Vec::new();
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

            let Some(widget) = widget_map.get(widget_key_name) else {
                continue;
            };
            if !widget.is_truncatable(widget_key) {
                continue;
            }

            rendered_widgets.push(RenderedWidget {
                part,
                widget_name: widget_key_name.to_owned(),
                name: widget_key.to_owned(),
                output: widget.process(widget_key, state),
            });
        }

        (output, rendered_widgets)
    }

    fn truncate_outputs(
        &self,
        output_left: &mut String,
        output_center: &mut String,
        output_right: &mut String,
        rendered_widgets: &[RenderedWidget],
        widget_map: &BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
        cols: usize,
    ) {
        let mut overflow = self
            .output_width(output_left, output_center, output_right, cols)
            .saturating_sub(cols);
        if overflow == 0 {
            return;
        }

        for part in self.format_precedence.iter().rev() {
            for rendered_widget in rendered_widgets
                .iter()
                .filter(|widget| widget.part == *part)
            {
                if overflow == 0 {
                    return;
                }

                let Some(widget) = widget_map.get(&rendered_widget.widget_name) else {
                    continue;
                };
                let current_width = console::measure_text_width(&rendered_widget.output);
                if current_width == 0 {
                    continue;
                }

                let max_width = current_width.saturating_sub(overflow);
                let truncated = widget.truncate(
                    &rendered_widget.name,
                    &rendered_widget.output,
                    max_width,
                    state,
                );
                let truncated_width = console::measure_text_width(&truncated);
                let reduced_by = current_width.saturating_sub(truncated_width);
                if reduced_by == 0 {
                    continue;
                }

                match rendered_widget.part {
                    Part::Left => {
                        *output_left = output_left.replacen(&rendered_widget.output, &truncated, 1)
                    }
                    Part::Center => {
                        *output_center =
                            output_center.replacen(&rendered_widget.output, &truncated, 1)
                    }
                    Part::Right => {
                        *output_right =
                            output_right.replacen(&rendered_widget.output, &truncated, 1)
                    }
                }
                overflow = overflow.saturating_sub(reduced_by);
            }
        }
    }

    fn output_width(
        &self,
        output_left: &str,
        output_center: &str,
        output_right: &str,
        cols: usize,
    ) -> usize {
        if output_center.is_empty() {
            return console::measure_text_width(output_left)
                + console::measure_text_width(output_right);
        }

        console::measure_text_width(output_left)
            + console::measure_text_width(&self.get_spacer_left(output_left, output_center, cols))
            + console::measure_text_width(output_center)
            + console::measure_text_width(&self.get_spacer_right(output_right, output_center, cols))
            + console::measure_text_width(output_right)
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
    use crate::widgets::pipe::PipeWidget;
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
    fn truncates_configured_pipe_to_keep_static_status_text_visible() {
        let mut config = BTreeMap::new();
        config.insert("format_left".to_owned(), "L".to_owned());
        config.insert("format_right".to_owned(), "{pipe_hints} CLOCK".to_owned());
        config.insert("pipe_hints_format".to_owned(), "{output}".to_owned());
        config.insert("pipe_hints_truncate".to_owned(), "true".to_owned());
        config.insert("pipe_hints_overflow".to_owned(), "...".to_owned());

        let mut module_config = ModuleConfig::new(&config).unwrap();
        let mut state = ZellijState {
            cols: 16,
            ..Default::default()
        };
        state.pipe_results.insert(
            "pipe_hints".to_owned(),
            "abcdefghijklmnopqrstuvwxyz".to_owned(),
        );

        let widget_map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "pipe".to_owned(),
            Arc::new(PipeWidget::new(&config)) as Arc<dyn Widget>,
        )]);

        let output = module_config.render_bar(state, widget_map);

        assert_eq!(console::measure_text_width(&output), 16);
        assert!(output.ends_with(" CLOCK"));
        assert!(output.contains("..."));
    }

    #[test]
    fn truncates_configured_pipe_to_empty_when_no_width_remains() {
        let mut config = BTreeMap::new();
        config.insert("format_left".to_owned(), "L".to_owned());
        config.insert("format_right".to_owned(), "{pipe_hints} CLOCK".to_owned());
        config.insert("pipe_hints_format".to_owned(), "{output}".to_owned());
        config.insert("pipe_hints_truncate".to_owned(), "true".to_owned());
        config.insert("pipe_hints_overflow".to_owned(), "...".to_owned());

        let mut module_config = ModuleConfig::new(&config).unwrap();
        let mut state = ZellijState {
            cols: 7,
            ..Default::default()
        };
        state.pipe_results.insert(
            "pipe_hints".to_owned(),
            "abcdefghijklmnopqrstuvwxyz".to_owned(),
        );

        let widget_map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "pipe".to_owned(),
            Arc::new(PipeWidget::new(&config)) as Arc<dyn Widget>,
        )]);

        let output = module_config.render_bar(state, widget_map);

        assert_eq!(console::measure_text_width(&output), 7);
        assert_eq!(output, "L CLOCK");
    }

    #[test]
    fn scrolling_pipe_pans_truncated_output() {
        let mut config = BTreeMap::new();
        config.insert("format_right".to_owned(), "{pipe_hints} CLOCK".to_owned());
        config.insert("pipe_hints_format".to_owned(), "{output}".to_owned());
        config.insert("pipe_hints_truncate".to_owned(), "true".to_owned());
        config.insert("pipe_hints_scrollable".to_owned(), "true".to_owned());
        config.insert("pipe_hints_overflow".to_owned(), "...".to_owned());

        let mut module_config = ModuleConfig::new(&config).unwrap();
        let mut state = ZellijState {
            cols: 16,
            ..Default::default()
        };
        state.pipe_results.insert(
            "pipe_hints".to_owned(),
            "abcdefghijklmnopqrstuvwxyz".to_owned(),
        );

        let widget_map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "pipe".to_owned(),
            Arc::new(PipeWidget::new(&config)) as Arc<dyn Widget>,
        )]);

        let initial = module_config.render_bar(state.clone(), widget_map.clone());
        assert_eq!(initial, "abcdefg... CLOCK");

        assert!(module_config.handle_mouse_action(
            &mut state,
            Mouse::ScrollDown(1),
            widget_map.clone(),
        ));

        let scrolled = module_config.render_bar(state, widget_map);
        assert_eq!(scrolled, "...efgh... CLOCK");
    }

    #[test]
    fn scrolling_pipe_offset_is_clamped_to_content_width() {
        let mut config = BTreeMap::new();
        config.insert("format_right".to_owned(), "{pipe_hints} CLOCK".to_owned());
        config.insert("pipe_hints_format".to_owned(), "{output}".to_owned());
        config.insert("pipe_hints_truncate".to_owned(), "true".to_owned());
        config.insert("pipe_hints_scrollable".to_owned(), "true".to_owned());

        let mut module_config = ModuleConfig::new(&config).unwrap();
        let mut state = ZellijState {
            cols: 16,
            ..Default::default()
        };
        state.pipe_results.insert(
            "pipe_hints".to_owned(),
            "abcdefghijklmnopqrstuvwxyz".to_owned(),
        );

        let widget_map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "pipe".to_owned(),
            Arc::new(PipeWidget::new(&config)) as Arc<dyn Widget>,
        )]);

        assert!(module_config.handle_mouse_action(&mut state, Mouse::ScrollDown(100), widget_map,));

        assert_eq!(state.pipe_scroll_offsets.get("pipe_hints"), Some(&25));
    }

    #[test]
    fn pipe_scroll_target_selects_which_scrollable_pipe_moves() {
        let mut config = BTreeMap::new();
        config.insert(
            "format_right".to_owned(),
            "{pipe_left} {pipe_right}".to_owned(),
        );
        config.insert("pipe_left_format".to_owned(), "{output}".to_owned());
        config.insert("pipe_left_scrollable".to_owned(), "true".to_owned());
        config.insert("pipe_right_format".to_owned(), "{output}".to_owned());
        config.insert("pipe_right_scrollable".to_owned(), "true".to_owned());
        config.insert("pipe_scroll_target".to_owned(), "pipe_right".to_owned());

        let mut module_config = ModuleConfig::new(&config).unwrap();
        let mut state = ZellijState {
            cols: 16,
            ..Default::default()
        };
        state
            .pipe_results
            .insert("pipe_left".to_owned(), "left".to_owned());
        state
            .pipe_results
            .insert("pipe_right".to_owned(), "right".to_owned());

        let widget_map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "pipe".to_owned(),
            Arc::new(PipeWidget::new(&config)) as Arc<dyn Widget>,
        )]);

        assert!(module_config.handle_mouse_action(&mut state, Mouse::ScrollDown(1), widget_map,));

        assert_eq!(state.pipe_scroll_offsets.get("pipe_left"), None);
        assert_eq!(state.pipe_scroll_offsets.get("pipe_right"), Some(&4));
    }
}
