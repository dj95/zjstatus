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
    pub plugin_uuid: String,
    pub tabs: Vec<TabInfo>,
    pub sessions: Vec<SessionInfo>,
    pub start_time: DateTime<Local>,
}

#[derive(Default)]
pub struct ModuleConfig {
    pub left_parts_config: String,
    pub left_parts: Vec<FormattedPart>,
    pub right_parts_config: String,
    pub right_parts: Vec<FormattedPart>,
    pub format_space: FormattedPart,
    pub hide_frame_for_single_pane: bool,
    pub border: BorderConfig,
}

impl ModuleConfig {
    pub fn new(config: BTreeMap<String, String>) -> Self {
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

        let border_config = match parse_border_config(config.clone()) {
            Some(bc) => bc,
            None => BorderConfig::default(),
        };

        Self {
            left_parts_config: left_parts_config.to_owned(),
            left_parts: parts_from_config(Some(&left_parts_config.to_owned())),
            right_parts_config: right_parts_config.to_owned(),
            right_parts: parts_from_config(Some(&right_parts_config.to_owned())),
            format_space: FormattedPart::from_format_string(format_space_config.to_owned()),
            hide_frame_for_single_pane,
            border: border_config,
        }
    }

    pub fn handle_mouse_action(
        &self,
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

        let widget_string_left = self
            .left_parts
            .iter()
            .map(|p| p.content.clone())
            .fold(String::new(), |a, b| a + b.as_str());

        let left_len = self.process_widget_click(
            click_pos,
            widget_string_left,
            widget_map.clone(),
            &state,
            0,
        );

        if click_pos <= left_len {
            return;
        }

        let output_right = self.right_parts.iter().fold("".to_owned(), |acc, part| {
            format!(
                "{}{}",
                acc,
                part.format_string_with_widgets(&widget_map, &state)
            )
        });

        let widget_spacer = strip_ansi_escapes::strip_str(self.get_spacer(
            " ".repeat(left_len),
            output_right,
            state.cols,
        ));

        let widget_string_right = self
            .right_parts
            .iter()
            .map(|p| p.content.clone())
            .fold(String::new(), |a, b| a + b.as_str());

        self.process_widget_click(
            click_pos,
            widget_string_right,
            widget_map,
            &state,
            left_len + widget_spacer.len(),
        );
    }

    fn process_widget_click(
        &self,
        click_pos: usize,
        widget_string: String,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
        state: &ZellijState,
        offset: usize,
    ) -> usize {
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
                Some(pos) => pos,
                None => continue,
            };

            let wid_res = strip_ansi_escapes::strip_str(wid.process(wid_name, state));
            rendered_output = rendered_output.replace(match_name, &wid_res);

            if click_pos < pos + offset || click_pos > pos + offset + wid_res.len() {
                continue;
            }

            wid.process_click(state, click_pos - pos + offset);
        }

        rendered_output.len()
    }

    pub fn render_bar(&self, state: ZellijState, widget_map: BTreeMap<String, Arc<dyn Widget>>) {
        let output_left = self.left_parts.iter().fold("".to_owned(), |acc, part| {
            format!(
                "{acc}{}",
                part.format_string_with_widgets(&widget_map, &state)
            )
        });

        let output_right = self.right_parts.iter().fold("".to_owned(), |acc, part| {
            format!(
                "{acc}{}",
                part.format_string_with_widgets(&widget_map, &state)
            )
        });

        let mut border_top = "".to_owned();
        if self.border.enabled && self.border.position == BorderPosition::Top {
            border_top = format!("{}\n", self.border.draw(state.cols));
        }

        let mut border_bottom = "".to_owned();
        if self.border.enabled && self.border.position == BorderPosition::Bottom {
            border_bottom = format!("\n{}", self.border.draw(state.cols));
        }

        print!(
            "{}{}{}{}{}",
            border_top,
            output_left,
            self.get_spacer(output_left.clone(), output_right.clone(), state.cols),
            output_right,
            border_bottom,
        );
    }

    fn get_spacer(&self, output_left: String, output_right: String, cols: usize) -> String {
        let text_count = strip_ansi_escapes::strip_str(output_left).chars().count()
            + strip_ansi_escapes::strip_str(output_right).chars().count();

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
            .map(|f| FormattedPart::from_format_string(f.to_owned()))
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

        let part = FormattedPart::from_format_string(input.to_owned());

        assert_eq!(
            part,
            FormattedPart {
                fg: Some(RgbColor(255, 0, 0).into()),
                bg: Some(RgbColor(0, 255, 0).into()),
                bold: true,
                italic: true,
                content: "foo".to_owned(),
            },
        )
    }
}
