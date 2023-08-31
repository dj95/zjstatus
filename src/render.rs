use std::{collections::BTreeMap, sync::Arc};

use ansi_term::Style;

use crate::{config::FormattedPart, widgets::widget::Widget, ZellijState};

pub fn formatting(part: FormattedPart, text: String) -> String {
    let mut style = match part.fg {
        Some(color) => Style::new().fg(color),
        None => Style::new(),
    };

    style.background = part.bg;
    style.is_italic = part.italic;
    style.is_bold = part.bold;

    let style = style.paint(text);

    format!("{}", style)
}

pub fn widgets_and_formatting(
    part: FormattedPart,
    widgets: BTreeMap<String, Arc<dyn Widget>>,
    state: ZellijState,
) -> String {
    let mut output = part.content.clone();

    if output.contains("{clock}") {
        let result = match widgets.get("clock") {
            Some(widget) => widget.process(state.clone()),
            None => "Use of uninitialized widget".to_string(),
        };

        output = output.replace("{clock}", &result);
    }

    if output.contains("{mode}") {
        let result = match widgets.get("mode") {
            Some(widget) => widget.process(state.clone()),
            None => "Use of uninitialized widget".to_string(),
        };

        output = output.replace("{mode}", &result);
    }

    if output.contains("{tabs}") {
        let result = match widgets.get("tabs") {
            Some(widget) => widget.process(state),
            None => "Use of uninitialized widget".to_string(),
        };

        output = output.replace("{tabs}", &result);
    }

    formatting(part, output)
}
