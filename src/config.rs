use std::{collections::BTreeMap, sync::Arc};

use regex::Regex;
use zellij_tile::prelude::*;

use crate::{
    border::{parse_border_config, BorderConfig, BorderPosition},
    render::FormattedPart,
    widgets::{command::CommandResult, widget::Widget},
};
use chrono::{DateTime, Local};

#[derive(Default, Debug, Clone)]
pub struct ZellijState {
    pub cols: usize,
    pub command_results: BTreeMap<String, CommandResult>,
    pub mode: ModeInfo,
    pub panes: PaneManifest,
    pub plugin_uuid: String,
    pub tabs: Vec<TabInfo>,
    pub sessions: Vec<SessionInfo>,
    pub start_time: DateTime<Local>,
    pub cache_mask: u8,
}

pub enum UpdateEventMask {
    Always = 0b10000000,
    Mode = 0b00000001,
    Tab = 0b00000010,
    Command = 0b00000100,
    Session = 0b00001000,
    None = 0b00000000,
}

pub fn event_mask_from_widget_name(name: &str) -> u8 {
    match name {
        "command" => UpdateEventMask::Always as u8,
        "datetime" => UpdateEventMask::Always as u8,
        "mode" => UpdateEventMask::Mode as u8,
        "session" => UpdateEventMask::Session as u8,
        "swap_layout" => UpdateEventMask::Tab as u8,
        "tabs" => UpdateEventMask::Tab as u8,
        _ => UpdateEventMask::None as u8,
    }
}

#[derive(Default)]
pub struct ModuleConfig {
    pub left_parts_config: String,
    pub left_parts: Vec<FormattedPart>,
    pub center_parts_config: String,
    pub center_parts: Vec<FormattedPart>,
    pub right_parts_config: String,
    pub right_parts: Vec<FormattedPart>,
    pub format_space: FormattedPart,
    pub hide_frame_for_single_pane: bool,
    pub border: BorderConfig,
}

impl ModuleConfig {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let format_space_config = match config.get("format_space") {
            Some(space_config) => space_config,
            None => "",
        };

        let hide_frame_for_single_pane = match config.get("hide_frame_for_single_pane") {
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

        let border_config = match parse_border_config(config.clone()) {
            Some(bc) => bc,
            None => BorderConfig::default(),
        };

        Self {
            left_parts_config: left_parts_config.to_owned(),
            left_parts: parts_from_config(Some(&left_parts_config.to_owned())),
            center_parts_config: center_parts_config.to_owned(),
            center_parts: parts_from_config(Some(&center_parts_config.to_owned())),
            right_parts_config: right_parts_config.to_owned(),
            right_parts: parts_from_config(Some(&right_parts_config.to_owned())),
            format_space: FormattedPart::from_format_string(format_space_config),
            hide_frame_for_single_pane,
            border: border_config,
        }
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
        };

        let output_left = self.left_parts.iter_mut().fold("".to_owned(), |acc, part| {
            format!(
                "{}{}",
                acc,
                part.format_string_with_widgets(&widget_map, &state)
            )
        });

        let mut offset = console::measure_text_width(&output_left);

        self.process_widget_click(click_pos, &self.left_parts, &widget_map, &state, 0);

        if click_pos <= offset {
            return;
        }

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

        if !output_center.is_empty() {
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

        let tokens: Vec<String> = widget_map.keys().map(|k| format!("{{{k}}}")).collect();

        let widgets_regex = Regex::new("(\\{[a-z_]+\\})").unwrap();
        for widget in widgets_regex.captures_iter(widget_string.as_str()) {
            let match_name = widget.get(0).unwrap().as_str();
            if !tokens.contains(&match_name.to_owned()) {
                continue;
            }

            let wid_name = match_name.trim_matches(|c| c == '{' || c == '}');

            let wid = match widget_map.get(wid_name) {
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

            let wid_res = wid.process(wid_name, state);
            rendered_output = rendered_output.replace(match_name, &wid_res);

            if click_pos < pos + offset
                || click_pos > pos + offset + console::measure_text_width(&wid_res)
            {
                continue;
            }

            wid.process_click(state, click_pos - (pos + offset));
        }

        console::measure_text_width(&rendered_output)
    }

    pub fn render_bar(
        &mut self,
        state: ZellijState,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
    ) -> String {
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

    fn get_spacer_left(&self, output_left: &str, output_center: &str, cols: usize) -> String {
        let text_count = console::measure_text_width(output_left)
            + (console::measure_text_width(output_center) as f32 / 2.0).floor() as usize;

        let center_pos = (cols as f32 / 2.0).floor() as usize;

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = center_pos.saturating_sub(text_count);

        eprintln!("space_count: {:?}", space_count);
        self.format_space.format_string(&" ".repeat(space_count))
    }

    fn get_spacer_right(&self, output_right: &str, output_center: &str, cols: usize) -> String {
        let text_count = console::measure_text_width(output_right)
            + (console::measure_text_width(output_center) as f32 / 2.0).ceil() as usize;

        let center_pos = (cols as f32 / 2.0).ceil() as usize;

        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        let space_count = center_pos.saturating_sub(text_count);

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

fn parts_from_config(format: Option<&String>) -> Vec<FormattedPart> {
    match format {
        Some(format) => format
            .split("#[")
            .map(FormattedPart::from_format_string)
            .collect(),
        None => vec![],
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anstyle::RgbColor;

    #[test]
    fn test_formatted_part_from_string() {
        let input = "#[fg=#ff0000,bg=#00ff00,bold,italic]foo";

        let part = FormattedPart::from_format_string(input);

        assert_eq!(
            part,
            FormattedPart {
                fg: Some(RgbColor(255, 0, 0).into()),
                bg: Some(RgbColor(0, 255, 0).into()),
                bold: true,
                italic: true,
                content: "foo".to_owned(),
                ..Default::default()
            },
        )
    }
}
