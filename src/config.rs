use std::{collections::BTreeMap, sync::Arc};

use regex::Regex;
use zellij_tile::prelude::Mouse;

use crate::{render::FormattedPart, widgets::widget::Widget, ZellijState};

#[derive(Default)]
pub struct ModuleConfig {
    pub left_parts_config: String,
    pub left_parts: Vec<FormattedPart>,
    pub right_parts_config: String,
    pub right_parts: Vec<FormattedPart>,
    pub format_space: FormattedPart,
    pub hide_frame_for_single_pane: bool,
}

impl ModuleConfig {
    pub fn new(config: BTreeMap<String, String>) -> Self {
        let mut format_space_config = "";
        if let Some(space_config) = config.get("format_space") {
            format_space_config = space_config;
        }

        let mut hide_frame_for_single_pane = false;
        if let Some(toggle) = config.get("hide_frame_for_single_pane") {
            hide_frame_for_single_pane = toggle == "true";
        }

        let mut left_parts_config = "";
        if let Some(conf) = config.get("format_left") {
            left_parts_config = conf;
        }

        let mut right_parts_config = "";
        if let Some(conf) = config.get("format_right") {
            right_parts_config = conf;
        }

        Self {
            left_parts_config: left_parts_config.to_string(),
            left_parts: parts_from_config(Some(&left_parts_config.to_string())),
            right_parts_config: right_parts_config.to_string(),
            right_parts: parts_from_config(Some(&right_parts_config.to_string())),
            format_space: FormattedPart::from_format_string(format_space_config.to_string()),
            hide_frame_for_single_pane,
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
            state.clone(),
            0,
        );

        if click_pos <= left_len {
            return;
        }

        let mut output_right = "".to_string();
        for part in self.right_parts.iter() {
            output_right = format!(
                "{}{}",
                output_right,
                part.format_string_with_widgets(widget_map.clone(), state.clone())
            );
        }

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
            state,
            left_len + widget_spacer.len(),
        );
    }

    fn process_widget_click(
        &self,
        click_pos: usize,
        widget_string: String,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
        state: ZellijState,
        offset: usize,
    ) -> usize {
        let mut rendered_output = widget_string.clone();

        let tokens: Vec<String> = widget_map.keys().map(|k| format!("{{{k}}}")).collect();

        let widgets_regex = Regex::new("(\\{[a-z]+\\})").unwrap();
        for widget in widgets_regex.captures_iter(widget_string.as_str()) {
            let match_name = widget.get(0).unwrap().as_str();
            if !tokens.contains(&match_name.to_string()) {
                continue;
            }

            let mut wid_name = match_name.replace('{', "");
            wid_name = wid_name.replace('}', "");

            let wid = widget_map.get(&wid_name);
            if wid.is_none() {
                continue;
            }
            let wid = wid.unwrap();

            let pos = rendered_output.find(match_name);
            if pos.is_none() {
                continue;
            }
            let pos = pos.unwrap();

            let wid_res = strip_ansi_escapes::strip_str(wid.process(state.clone()));
            rendered_output = rendered_output.replace(match_name, &wid_res);

            if click_pos < pos + offset || click_pos > pos + offset + wid_res.len() {
                continue;
            }

            wid.process_click(state.clone(), click_pos - pos + offset);

        }

        rendered_output.len()
    }

    pub fn render_bar(&self, state: ZellijState, widget_map: BTreeMap<String, Arc<dyn Widget>>) {
        let mut output_left = "".to_string();
        for part in self.left_parts.iter() {
            output_left = format!(
                "{}{}",
                output_left,
                part.format_string_with_widgets(widget_map.clone(), state.clone())
            );
        }

        let mut output_right = "".to_string();
        for part in self.right_parts.iter() {
            output_right = format!(
                "{}{}",
                output_right,
                part.format_string_with_widgets(widget_map.clone(), state.clone())
            );
        }

        print!(
            "{}{}{}",
            output_left,
            self.get_spacer(output_left.clone(), output_right.clone(), state.cols),
            output_right
        );
    }

    fn get_spacer(&self, output_left: String, output_right: String, cols: usize) -> String {
        let text_count = strip_ansi_escapes::strip(output_left).len()
            + strip_ansi_escapes::strip(output_right).len();

        let mut space_count = cols;
        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        if space_count > text_count {
            space_count -= text_count;
        }

        self.format_space.format_string(" ".repeat(space_count))
    }
}

fn parts_from_config(format: Option<&String>) -> Vec<FormattedPart> {
    if format.is_none() {
        return Vec::new();
    }

    let mut output = Vec::new();

    let format_left = format.unwrap();

    let color_parts = format_left.split("#[");
    for color_part in color_parts {
        let part = FormattedPart::from_format_string(color_part.to_string());

        output.push(part);
    }

    output
}

#[cfg(test)]
mod test {
    use super::*;
    use anstyle::RgbColor;

    #[test]
    fn test_formatted_part_from_string() {
        let input = "#[fg=#ff0000,bg=#00ff00,bold,italic]foo";

        let part = FormattedPart::from_format_string(input.to_string());

        assert_eq!(
            part,
            FormattedPart {
                fg: Some(RgbColor(255, 0, 0).into()),
                bg: Some(RgbColor(0, 255, 0).into()),
                bold: true,
                italic: true,
                content: "foo".to_string(),
            },
        )
    }
}
