use std::{collections::BTreeMap, sync::Arc};

use crate::{render::FormattedPart, widgets::widget::Widget, ZellijState};

#[derive(Default)]
pub struct ModuleConfig {
    pub left_parts: Vec<FormattedPart>,
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

        Self {
            left_parts: parts_from_config(config.get("format_left")),
            right_parts: parts_from_config(config.get("format_right")),
            format_space: FormattedPart::from_format_string(format_space_config.to_string()),
            hide_frame_for_single_pane,
        }
    }

    pub fn render_bar(
        &self,
        state: ZellijState,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
        cols: usize,
    ) {
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

        let text_count = strip_ansi_escapes::strip(output_left.clone()).len()
            + strip_ansi_escapes::strip(output_right.clone()).len();

        let mut space_count = cols;
        // verify we are able to count the difference, since zellij sometimes drops a col
        // count of 0 on tab creation
        if space_count > text_count {
            space_count -= text_count;
        }

        let spaces = self.format_space.format_string(" ".repeat(space_count));

        print!("{}{}{}", output_left, spaces, output_right);
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
    use ansi_term::Colour;

    #[test]
    fn test_formatted_part_from_string() {
        let input = "#[fg=#ff0000,bg=#00ff00,bold,italic]foo";

        let part = FormattedPart::from_format_string(input.to_string());

        assert_eq!(
            part,
            FormattedPart {
                fg: Some(Colour::RGB(255, 0, 0)),
                bg: Some(Colour::RGB(0, 255, 0)),
                bold: true,
                italic: true,
                content: "foo".to_string(),
            },
        )
    }
}
