//! Agent status from Zellij's attention tracking (Zellij 0.46+), which programs
//! feed through the OSC 7501 program status protocol.

use std::collections::BTreeMap;

use zellij_tile::prelude::{
    AttentionCounts, AttentionSource, AttentionStatus, PaneManifest, TabInfo,
};

use crate::render::FormattedPart;

pub const ATTENTION_TOKEN: &str = "{attention}";
pub const ATTENTION_COUNTS_TOKEN: &str = "{attention_counts}";

/// The statuses that can be shown, most urgent first: the order of icons in
/// counts, and the order in which `{attention}` picks a tab's status.
pub const STATUSES: [AttentionStatus; 6] = [
    AttentionStatus::NeedsYou,
    AttentionStatus::Failed,
    AttentionStatus::Done,
    AttentionStatus::Working,
    AttentionStatus::Quiet,
    AttentionStatus::NewOutput,
];

/// Statuses shown without configuration, with Zellij's own symbols.
const DEFAULT_STATUSES: [AttentionStatus; 4] = [
    AttentionStatus::NeedsYou,
    AttentionStatus::Failed,
    AttentionStatus::Done,
    AttentionStatus::Working,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    /// Zellij's summary of each tab: program statuses and its own heuristics
    /// (finished commands, new output, ...).
    All,
    /// Only statuses that programs report themselves (OSC 7501).
    ProgramStatus,
}

#[derive(Clone, Debug)]
pub struct AttentionConfig {
    icons: Vec<(AttentionStatus, Vec<FormattedPart>)>,
    count_format: String,
    count_separator: String,
    source: Source,
}

impl AttentionConfig {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let icons = STATUSES
            .iter()
            .filter_map(|status| {
                let format = match config.get(&format!("attention_{}", status.as_str())) {
                    Some(format) => format.to_owned(),
                    None if DEFAULT_STATUSES.contains(status) => status.symbol().to_owned(),
                    None => return None,
                };
                let parts = pieces(&format, config);
                (!parts.is_empty()).then_some((*status, parts))
            })
            .collect();

        let source = match config.get("attention_source").map(String::as_str) {
            Some("program_status") => Source::ProgramStatus,
            _ => Source::All,
        };

        Self {
            icons,
            count_format: config
                .get("attention_count_format")
                .cloned()
                .unwrap_or_else(|| "{icon}{count}".to_owned()),
            count_separator: config
                .get("attention_count_separator")
                .cloned()
                .unwrap_or_else(|| " ".to_owned()),
            source,
        }
    }

    /// How many panes of the tab have each status.
    pub fn tab_counts(&self, tab: &TabInfo, panes: &PaneManifest) -> AttentionCounts {
        match self.source {
            Source::All => tab.attention.counts,
            Source::ProgramStatus => {
                let mut counts = AttentionCounts::default();
                for pane in panes.panes.get(&tab.position).into_iter().flatten() {
                    if !pane.is_plugin
                        && pane.attention.source == Some(AttentionSource::ProgramStatus)
                    {
                        counts.add(&pane.attention);
                    }
                }
                counts
            }
        }
    }

    pub fn total_counts<'a>(
        &self,
        tabs: impl IntoIterator<Item = &'a TabInfo>,
        panes: &PaneManifest,
    ) -> AttentionCounts {
        let mut total = AttentionCounts::default();
        for tab in tabs {
            total.merge(&self.tab_counts(tab, panes));
        }
        total
    }

    /// The most urgent status in `counts` that has an icon.
    pub fn top(&self, counts: &AttentionCounts) -> Option<AttentionStatus> {
        self.icons
            .iter()
            .map(|(status, _)| *status)
            .find(|status| counts.of(*status) > 0)
    }

    /// The icon of the most urgent status, for `{attention}`.
    pub fn icon(&self, counts: &AttentionCounts) -> Vec<FormattedPart> {
        self.top(counts)
            .and_then(|top| self.icons.iter().find(|(status, _)| *status == top))
            .map(|(_, icon)| icon.clone())
            .unwrap_or_default()
    }

    /// Each status with an icon and a count, e.g. "●2 ◐1", for
    /// `{attention_counts}`. A count takes the style of its icon's last part.
    pub fn counts(&self, counts: &AttentionCounts) -> Vec<FormattedPart> {
        let mut output = vec![];
        for (status, icon) in &self.icons {
            let count = counts.of(*status);
            if count == 0 {
                continue;
            }
            if !output.is_empty() && !self.count_separator.is_empty() {
                output.push(plain(&self.count_separator));
            }
            let count_style = icon.last().cloned().unwrap_or_default();
            for chunk in split_tokens(&self.count_format, &["{icon}", "{count}"]) {
                match chunk {
                    Chunk::Text(text) => output.push(FormattedPart {
                        content: text.to_owned(),
                        ..count_style.clone()
                    }),
                    Chunk::Token(0) => output.extend(icon.iter().cloned()),
                    Chunk::Token(_) => output.push(FormattedPart {
                        content: count.to_string(),
                        ..count_style.clone()
                    }),
                }
            }
        }
        output
    }

    /// Renders `content` in the style of `base`, replacing `{attention}` and
    /// `{attention_counts}` with styled icons. The counts are only computed
    /// when one of the placeholders is used.
    pub fn render(
        &self,
        base: &FormattedPart,
        content: &str,
        counts: impl FnOnce() -> AttentionCounts,
    ) -> String {
        if !has_tokens(content) {
            return base.format_string(content);
        }
        let counts = counts();
        let icon = self.icon(&counts);
        let count_parts = self.counts(&counts);
        render_with_tokens(
            base,
            content,
            &[
                (ATTENTION_TOKEN, &icon),
                (ATTENTION_COUNTS_TOKEN, &count_parts),
            ],
        )
    }
}

pub fn has_tokens(content: &str) -> bool {
    content.contains(ATTENTION_TOKEN) || content.contains(ATTENTION_COUNTS_TOKEN)
}

/// The non-empty parts of a format string such as "#[fg=red]●".
pub fn pieces(format: &str, config: &BTreeMap<String, String>) -> Vec<FormattedPart> {
    FormattedPart::multiple_from_format_string(format, config)
        .into_iter()
        .filter(|part| !part.content.is_empty())
        .collect()
}

fn plain(text: &str) -> FormattedPart {
    FormattedPart {
        content: text.to_owned(),
        ..FormattedPart::default()
    }
}

/// `overlay`'s style, with what it leaves unset taken from `base`.
pub fn inherit(base: &FormattedPart, overlay: &FormattedPart) -> FormattedPart {
    FormattedPart {
        fg: overlay.fg.or(base.fg),
        bg: overlay.bg.or(base.bg),
        us: overlay.us.or(base.us),
        effects: base.effects | overlay.effects,
        content: overlay.content.clone(),
        ..FormattedPart::default()
    }
}

/// Styled parts rendered inside a part styled `base`.
pub fn render_parts(base: &FormattedPart, parts: &[FormattedPart]) -> String {
    parts
        .iter()
        .map(|part| {
            let style = inherit(base, part);
            style.format_string(&style.content)
        })
        .collect()
}

/// Renders `content` in the style of `base`, replacing each token with its
/// styled parts.
pub fn render_with_tokens(
    base: &FormattedPart,
    content: &str,
    tokens: &[(&str, &[FormattedPart])],
) -> String {
    let names: Vec<&str> = tokens.iter().map(|(name, _)| *name).collect();
    split_tokens(content, &names)
        .into_iter()
        .map(|chunk| match chunk {
            Chunk::Text(text) => base.format_string(text),
            Chunk::Token(index) => render_parts(base, tokens[index].1),
        })
        .collect()
}

#[derive(Debug, PartialEq)]
pub enum Chunk<'a> {
    Text(&'a str),
    /// The index of the token in the list given to `split_tokens`.
    Token(usize),
}

/// Splits `content` into text and tokens, in order. Empty text is left out.
pub fn split_tokens<'a>(content: &'a str, tokens: &[&str]) -> Vec<Chunk<'a>> {
    let mut chunks = vec![];
    let mut rest = content;
    loop {
        let next = tokens
            .iter()
            .enumerate()
            .filter_map(|(index, token)| rest.find(token).map(|at| (at, index, token.len())))
            .min_by_key(|(at, _, _)| *at);
        match next {
            Some((at, index, len)) => {
                if at > 0 {
                    chunks.push(Chunk::Text(&rest[..at]));
                }
                chunks.push(Chunk::Token(index));
                rest = &rest[at + len..];
            }
            None => {
                if !rest.is_empty() {
                    chunks.push(Chunk::Text(rest));
                }
                return chunks;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anstyle::{Effects, RgbColor};
    use std::collections::HashMap;
    use zellij_tile::prelude::{MuteScope, PaneAttention, PaneInfo, TabAttention};

    fn config(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    fn counts(needs_you: usize, failed: usize, done: usize, working: usize) -> AttentionCounts {
        AttentionCounts {
            needs_you,
            failed,
            done,
            working,
            ..AttentionCounts::default()
        }
    }

    fn text(parts: &[FormattedPart]) -> String {
        parts.iter().map(|part| part.content.as_str()).collect()
    }

    fn pane(id: u32, status: AttentionStatus, source: AttentionSource) -> PaneInfo {
        PaneInfo {
            id,
            attention: PaneAttention {
                status,
                source: Some(source),
                ..PaneAttention::default()
            },
            ..PaneInfo::default()
        }
    }

    #[test]
    fn defaults_to_zellij_symbols_for_the_counted_statuses() {
        let attention = AttentionConfig::new(&BTreeMap::new());
        let all = AttentionCounts {
            quiet: 1,
            new_output: 1,
            ..counts(1, 1, 1, 1)
        };

        assert_eq!(text(&attention.counts(&all)), "●1 ✗1 ✓1 ◐1");
        assert_eq!(text(&attention.icon(&counts(0, 0, 1, 3))), "✓");
        assert_eq!(text(&attention.icon(&AttentionCounts::default())), "");
    }

    #[test]
    fn shows_only_statuses_with_an_icon() {
        let attention = AttentionConfig::new(&config(&[
            ("attention_done", ""),
            ("attention_new_output", "•"),
        ]));
        let some = AttentionCounts {
            new_output: 2,
            ..counts(0, 0, 4, 1)
        };

        assert_eq!(text(&attention.counts(&some)), "◐1 •2");
        assert_eq!(attention.top(&counts(0, 0, 4, 0)), None);
    }

    #[test]
    fn counts_take_the_style_of_their_icon() {
        let attention = AttentionConfig::new(&config(&[
            ("attention_working", "#[fg=#f9e2af]W"),
            ("attention_needs_you", "#[fg=#fab387]N"),
            ("attention_count_format", "{icon} {count}"),
            ("attention_count_separator", "  "),
        ]));

        let parts = attention.counts(&counts(1, 0, 0, 2));

        assert_eq!(text(&parts), "N 1  W 2");
        let yellow = Some(RgbColor(0xf9, 0xe2, 0xaf).into());
        let count = parts.last().unwrap();
        assert_eq!((count.content.as_str(), count.fg), ("2", yellow));
        let separator = parts.iter().find(|part| part.content == "  ").unwrap();
        assert_eq!(separator.fg, None);
    }

    #[test]
    fn icons_inherit_the_unset_style_of_the_surrounding_part() {
        let base =
            FormattedPart::from_format_string("#[fg=#6c7086,bg=#181825,bold] x", &BTreeMap::new());
        let icon = FormattedPart::from_format_string("#[fg=#a6e3a1]✓", &BTreeMap::new());

        let style = inherit(&base, &icon);

        assert_eq!(style.fg, Some(RgbColor(0xa6, 0xe3, 0xa1).into()));
        assert_eq!(style.bg, Some(RgbColor(0x18, 0x18, 0x25).into()));
        assert_eq!(style.effects, Effects::BOLD);
        assert_eq!(style.content, "✓");
    }

    #[test]
    fn renders_placeholders_with_icon_styles_and_the_rest_with_the_part_style() {
        let attention = AttentionConfig::new(&config(&[("attention_working", "#[fg=#f9e2af]W")]));
        let base = FormattedPart::from_format_string("#[fg=#6c7086] x", &BTreeMap::new());
        let icon = inherit(
            &base,
            &FormattedPart::from_format_string("#[fg=#f9e2af]W", &BTreeMap::new()),
        );

        let rendered = attention.render(&base, "{attention} api ", || counts(0, 0, 0, 1));

        assert_eq!(
            rendered,
            format!("{}{}", icon.format_string("W"), base.format_string(" api ")),
        );
        assert_eq!(
            attention.render(&base, " api ", || counts(0, 0, 0, 1)),
            base.format_string(" api "),
            "without placeholders, a part renders as upstream does",
        );
    }

    #[test]
    fn program_status_counts_only_panes_that_reported_themselves() {
        let attention = AttentionConfig::new(&config(&[("attention_source", "program_status")]));
        let tab = TabInfo {
            position: 1,
            attention: TabAttention {
                counts: counts(0, 0, 3, 3),
                ..TabAttention::default()
            },
            ..TabInfo::default()
        };
        let mut muted = pane(4, AttentionStatus::NeedsYou, AttentionSource::ProgramStatus);
        muted.attention.muted = Some(MuteScope::Pane);
        let panes = PaneManifest {
            panes: HashMap::from([(
                1,
                vec![
                    pane(1, AttentionStatus::Working, AttentionSource::ProgramStatus),
                    pane(2, AttentionStatus::Done, AttentionSource::ShellCommand),
                    pane(3, AttentionStatus::NeedsYou, AttentionSource::ProgramStatus),
                    muted,
                ],
            )]),
        };

        let program = attention.tab_counts(&tab, &panes);
        let all = AttentionConfig::new(&BTreeMap::new()).tab_counts(&tab, &panes);

        assert_eq!(
            (program.needs_you, program.working, program.done),
            (1, 1, 0)
        );
        assert_eq!(all, counts(0, 0, 3, 3));
    }

    #[test]
    fn splits_text_and_tokens_in_order() {
        assert_eq!(
            split_tokens("a{x}{y}b{x}", &["{x}", "{y}"]),
            vec![
                Chunk::Text("a"),
                Chunk::Token(0),
                Chunk::Token(1),
                Chunk::Text("b"),
                Chunk::Token(0),
            ],
        );
        assert_eq!(split_tokens("", &["{x}"]), vec![]);
    }
}
