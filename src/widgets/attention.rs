use std::collections::BTreeMap;

use crate::{
    attention::{AttentionConfig, render_with_tokens},
    config::ZellijState,
    render::FormattedPart,
};

use super::widget::Widget;

/// `{attention}`: how many panes of the whole session have each status, e.g.
/// "●1 ◐3". Empty when there is nothing to show.
pub struct AttentionWidget {
    config: AttentionConfig,
    format: Vec<FormattedPart>,
}

impl AttentionWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let format = config
            .get("attention_format")
            .map(String::as_str)
            .unwrap_or("{counts}");

        Self {
            config: AttentionConfig::new(config),
            format: FormattedPart::multiple_from_format_string(format, config),
        }
    }
}

impl Widget for AttentionWidget {
    fn process(&self, _name: &str, state: &ZellijState) -> String {
        let counts = self.config.total_counts(&state.tabs, &state.panes);
        let count_parts = self.config.counts(&counts);
        if count_parts.is_empty() {
            return "".to_owned();
        }
        let icon = self.config.icon(&counts);

        self.format
            .iter()
            .map(|part| {
                render_with_tokens(
                    part,
                    &part.content,
                    &[("{counts}", &count_parts), ("{icon}", &icon)],
                )
            })
            .collect()
    }

    fn process_click(&self, _name: &str, _state: &ZellijState, _pos: usize) {}
}

#[cfg(test)]
mod test {
    use super::*;
    use zellij_tile::prelude::{AttentionCounts, TabAttention, TabInfo};

    fn tab(working: usize, needs_you: usize) -> TabInfo {
        TabInfo {
            attention: TabAttention {
                counts: AttentionCounts {
                    working,
                    needs_you,
                    ..AttentionCounts::default()
                },
                ..TabAttention::default()
            },
            ..TabInfo::default()
        }
    }

    #[test]
    fn sums_all_tabs_and_is_empty_without_statuses() {
        let config = BTreeMap::from([("attention_format".to_owned(), " {counts} ".to_owned())]);
        let widget = AttentionWidget::new(&config);
        let base = FormattedPart::default();

        let busy = ZellijState {
            tabs: vec![tab(1, 0), tab(2, 1)],
            ..ZellijState::default()
        };
        let idle = ZellijState {
            tabs: vec![tab(0, 0)],
            ..ZellijState::default()
        };

        let expected = [" ", "●", "1", " ", "◐", "3", " "]
            .iter()
            .map(|text| base.format_string(text))
            .collect::<String>();
        assert_eq!(widget.process("attention", &busy), expected);
        assert_eq!(widget.process("attention", &idle), "");
    }
}
