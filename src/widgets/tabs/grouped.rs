//! The tab bar with tab groups (see `crate::tab_groups`).

use std::{cmp, collections::BTreeMap};

use zellij_tile::prelude::{AttentionCounts, InputMode, TabInfo};

use crate::{
    attention::{render_parts, split_tokens, Chunk, ATTENTION_COUNTS_TOKEN, ATTENTION_TOKEN},
    config::ZellijState,
    render::FormattedPart,
    tab_groups::{self, CollapseMode, Group, GroupsConfig, Item},
};

use super::TabsWidget;

pub(super) struct GroupFormats {
    config: GroupsConfig,
    expanded: Vec<FormattedPart>,
    collapsed: Vec<FormattedPart>,
    collapsed_active: Vec<FormattedPart>,
    hidden: Vec<FormattedPart>,
    separator: Option<FormattedPart>,
}

impl GroupFormats {
    /// `None` unless tab groups are on.
    pub(super) fn new(config: &BTreeMap<String, String>) -> Option<Self> {
        let groups = GroupsConfig::new(config)?;
        let format = |key: &str| {
            config
                .get(key)
                .map(|format| FormattedPart::multiple_from_format_string(format, config))
        };
        let default = |format: &str| FormattedPart::multiple_from_format_string(format, config);

        let expanded = format("tab_group_expanded").unwrap_or_else(|| default(" {name}:"));
        Some(Self {
            config: groups,
            collapsed: format("tab_group_collapsed")
                .unwrap_or_else(|| default(" {name} +{count} {attention_counts} ")),
            collapsed_active: format("tab_group_collapsed_active")
                .unwrap_or_else(|| expanded.clone()),
            hidden: format("tab_group_hidden")
                .unwrap_or_else(|| default("+{count} {attention_counts} ")),
            expanded,
            separator: config
                .get("tab_group_separator")
                .map(|separator| FormattedPart::from_format_string(separator, config)),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Action {
    /// Go to the tab at this position.
    Switch(usize),
    /// Expand or collapse the group.
    Toggle(String),
}

#[derive(Debug)]
pub(super) struct Segment {
    text: String,
    action: Option<Action>,
}

impl TabsWidget {
    pub(super) fn process_grouped(&self, groups: &GroupFormats, state: &ZellijState) -> String {
        self.segments(groups, state)
            .into_iter()
            .map(|segment| segment.text)
            .collect()
    }

    pub(super) fn process_click_grouped(
        &self,
        groups: &GroupFormats,
        state: &ZellijState,
        pos: usize,
    ) {
        match self.clicked(groups, state, pos) {
            Some(Action::Switch(position)) => {
                #[cfg(all(not(feature = "bench"), not(test)))]
                zellij_tile::prelude::switch_tab_to(position as u32 + 1);
                #[cfg(any(feature = "bench", test))]
                let _ = position;
            }
            Some(Action::Toggle(group)) => {
                let collapsed = self.is_collapsed(groups, &group, state, false);
                #[cfg(all(not(feature = "bench"), not(test)))]
                tab_groups::store::set(state.mode.session_name.as_deref(), &group, !collapsed);
                #[cfg(any(feature = "bench", test))]
                let _ = collapsed;
            }
            None => {}
        }
    }

    /// What a mouse event at `pos` does. A click also sends a release, which
    /// must not undo a toggle, so only the press (left click) acts.
    fn clicked(&self, groups: &GroupFormats, state: &ZellijState, pos: usize) -> Option<Action> {
        if !state.tab_groups.left_click {
            return None;
        }
        self.click_action(groups, state, pos)
    }

    fn click_action(
        &self,
        groups: &GroupFormats,
        state: &ZellijState,
        pos: usize,
    ) -> Option<Action> {
        let mut offset = 0;
        for segment in self.segments(groups, state) {
            let width = console::measure_text_width(&segment.text);
            if pos >= offset && pos < offset + width {
                return segment.action;
            }
            offset += width;
        }
        None
    }

    fn is_collapsed(
        &self,
        groups: &GroupFormats,
        name: &str,
        state: &ZellijState,
        active: bool,
    ) -> bool {
        match groups.config.mode {
            CollapseMode::Auto => !active,
            CollapseMode::Manual => state
                .tab_groups
                .collapse
                .is_collapsed(name, groups.config.start_collapsed),
        }
    }

    fn segments(&self, groups: &GroupFormats, state: &ZellijState) -> Vec<Segment> {
        let items = tab_groups::build_items(&state.tabs, &groups.config);
        let active = items.iter().position(Item::is_active);
        let (start, end) = window(items.len(), active, self.tab_display_count);
        let mut segments = vec![];

        if start > 0 {
            let before = items[start - 1].tabs();
            let action = before.last().map(|tab| Action::Switch(tab.position));
            self.push_truncation(
                &self.tab_truncate_start_format,
                start,
                action,
                &mut segments,
            );
        }

        for index in start..end {
            if index > start {
                let near_group = matches!(items[index - 1], Item::Group(_))
                    || matches!(items[index], Item::Group(_));
                let separator = match near_group {
                    true => groups.separator.as_ref().or(self.separator.as_ref()),
                    false => self.separator.as_ref(),
                };
                push_separator(separator, &mut segments);
            }
            match &items[index] {
                Item::Tab(tab) => segments.push(Segment {
                    text: self.render_tab(tab, &state.panes, &state.mode),
                    action: Some(Action::Switch(tab.position)),
                }),
                Item::Group(group) => self.push_group(groups, group, state, &mut segments),
            }
        }

        if end < items.len() {
            let after = items[end].tabs();
            let action = after.first().map(|tab| Action::Switch(tab.position));
            self.push_truncation(
                &self.tab_truncate_end_format,
                items.len() - end,
                action,
                &mut segments,
            );
        }

        segments
    }

    fn push_truncation(
        &self,
        format: &[FormattedPart],
        count: usize,
        action: Option<Action>,
        segments: &mut Vec<Segment>,
    ) {
        let text: String = format
            .iter()
            .map(|part| part.format_string(&part.content.replace("{count}", &count.to_string())))
            .collect();
        segments.push(Segment { text, action });
    }

    fn push_group(
        &self,
        groups: &GroupFormats,
        group: &Group,
        state: &ZellijState,
        segments: &mut Vec<Segment>,
    ) {
        let active = group.active();
        let tabs: Vec<&TabInfo> = group.members.iter().map(|(tab, _)| *tab).collect();
        // A group of one tab never collapses, and its label goes to the tab.
        let collapsed =
            group.collapsible() && self.is_collapsed(groups, &group.name, state, active.is_some());
        let label_action = match groups.config.mode {
            _ if !group.collapsible() => tabs.first().map(|tab| Action::Switch(tab.position)),
            CollapseMode::Manual => Some(Action::Toggle(group.name.clone())),
            // Collapsing follows the active tab: clicking a group goes to it.
            CollapseMode::Auto => match collapsed {
                true => self
                    .urgent_tab(&tabs, state)
                    .or(tabs.first().copied())
                    .map(|tab| Action::Switch(tab.position)),
                false => None,
            },
        };

        if !collapsed {
            self.push_label(
                &groups.expanded,
                &group.name,
                &tabs,
                label_action,
                state,
                segments,
            );
            for (index, (tab, name)) in group.members.iter().enumerate() {
                if index > 0 {
                    push_separator(self.separator.as_ref(), segments);
                }
                self.push_member(tab, name, state, segments);
            }
            return;
        }

        let Some(active) = active else {
            self.push_label(
                &groups.collapsed,
                &group.name,
                &tabs,
                label_action,
                state,
                segments,
            );
            return;
        };
        self.push_label(
            &groups.collapsed_active,
            &group.name,
            &tabs,
            label_action.clone(),
            state,
            segments,
        );
        if let Some((tab, name)) = group.members.iter().find(|(tab, _)| tab.active) {
            self.push_member(tab, name, state, segments);
        }
        let hidden: Vec<&TabInfo> = tabs
            .into_iter()
            .filter(|tab| tab.tab_id != active.tab_id)
            .collect();
        if !hidden.is_empty() {
            self.push_label(
                &groups.hidden,
                &group.name,
                &hidden,
                label_action,
                state,
                segments,
            );
        }
    }

    /// A tab of a group, under its name without the prefix. While it is being
    /// renamed, it shows its full name.
    fn push_member(
        &self,
        tab: &TabInfo,
        name: &str,
        state: &ZellijState,
        segments: &mut Vec<Segment>,
    ) {
        let renaming = tab.active && state.mode.mode == InputMode::RenameTab;
        let shown = TabInfo {
            name: if renaming {
                tab.name.clone()
            } else {
                name.to_owned()
            },
            ..tab.clone()
        };
        segments.push(Segment {
            text: self.render_tab(&shown, &state.panes, &state.mode),
            action: Some(Action::Switch(tab.position)),
        });
    }

    /// A group label for `tabs`: `{name}`, `{count}`, `{attention}` and
    /// `{attention_counts}`. Clicking the counts goes to the most urgent tab.
    fn push_label(
        &self,
        format: &[FormattedPart],
        name: &str,
        tabs: &[&TabInfo],
        action: Option<Action>,
        state: &ZellijState,
        segments: &mut Vec<Segment>,
    ) {
        let counts = self.counts(tabs, state);
        let icon = self.attention.icon(&counts);
        let count_parts = self.attention.counts(&counts);
        let urgent = self
            .urgent_tab(tabs, state)
            .map(|tab| Action::Switch(tab.position))
            .or(action.clone());

        for part in format {
            let content = part
                .content
                .replace("{name}", name)
                .replace("{count}", &tabs.len().to_string());
            for chunk in split_tokens(&content, &[ATTENTION_TOKEN, ATTENTION_COUNTS_TOKEN]) {
                let segment = match chunk {
                    Chunk::Text(text) => Segment {
                        text: part.format_string(text),
                        action: action.clone(),
                    },
                    Chunk::Token(0) => Segment {
                        text: render_parts(part, &icon),
                        action: action.clone(),
                    },
                    Chunk::Token(_) => Segment {
                        text: render_parts(part, &count_parts),
                        action: urgent.clone(),
                    },
                };
                segments.push(segment);
            }
        }
    }

    fn counts(&self, tabs: &[&TabInfo], state: &ZellijState) -> AttentionCounts {
        self.attention
            .total_counts(tabs.iter().copied(), &state.panes)
    }

    /// The first of `tabs` with their most urgent status.
    fn urgent_tab<'a>(&self, tabs: &[&'a TabInfo], state: &ZellijState) -> Option<&'a TabInfo> {
        let top = self.attention.top(&self.counts(tabs, state))?;
        tabs.iter()
            .copied()
            .find(|tab| self.attention.tab_counts(tab, &state.panes).of(top) > 0)
    }
}

fn push_separator(separator: Option<&FormattedPart>, segments: &mut Vec<Segment>) {
    if let Some(separator) = separator {
        segments.push(Segment {
            text: separator.format_string(&separator.content),
            action: None,
        });
    }
}

/// The items to show when at most `max` fit, as `start..end`: the same
/// window as `get_tab_window`, over items instead of tabs.
fn window(len: usize, active: Option<usize>, max: Option<usize>) -> (usize, usize) {
    let Some(max) = max.filter(|max| len > *max) else {
        return (0, len);
    };
    let active = active.unwrap_or(0);
    if active > len.saturating_sub(max) {
        return (len.saturating_sub(max), len);
    }
    let first = active.saturating_sub(1);
    (first, cmp::min(first + max, len))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{tab_groups::CollapseState, widgets::widget::Widget};
    use zellij_tile::prelude::{ModeInfo, TabAttention};

    fn widget(extra: &[(&str, &str)]) -> TabsWidget {
        let mut config: BTreeMap<String, String> = [
            ("tab_groups", "true"),
            ("tab_normal", " {name} "),
            ("tab_active", "<{name}>"),
            ("tab_separator", "|"),
            ("tab_group_expanded", "{name}:"),
            ("tab_group_collapsed", "{name}+{count}{attention_counts}"),
            ("tab_group_hidden", "+{count}{attention_counts}"),
        ]
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
        for (key, value) in extra {
            config.insert(key.to_string(), value.to_string());
        }
        TabsWidget::new(&config)
    }

    /// Tabs from names; `*` marks the active tab and a trailing `~` a
    /// working pane.
    fn state(names: &[&str]) -> ZellijState {
        let tabs = names
            .iter()
            .enumerate()
            .map(|(position, name)| {
                let working = name.ends_with('~');
                let name = name.trim_end_matches('~');
                TabInfo {
                    position,
                    tab_id: position,
                    active: name.ends_with('*'),
                    name: name.trim_end_matches('*').to_owned(),
                    attention: TabAttention {
                        counts: AttentionCounts {
                            working: working as usize,
                            ..AttentionCounts::default()
                        },
                        ..TabAttention::default()
                    },
                    ..TabInfo::default()
                }
            })
            .collect();
        ZellijState {
            tabs,
            ..ZellijState::default()
        }
    }

    fn collapse(mut state: ZellijState, group: &str) -> ZellijState {
        state.tab_groups.collapse.set(group, true);
        state
    }

    fn shown(widget: &TabsWidget, state: &ZellijState) -> String {
        console::strip_ansi_codes(&widget.process("tabs", state)).to_string()
    }

    fn click(widget: &TabsWidget, state: &ZellijState, pos: usize) -> Option<Action> {
        let Some(groups) = &widget.groups else {
            panic!("groups are off");
        };
        widget.click_action(groups, state, pos)
    }

    #[test]
    fn shows_groups_and_plain_tabs() {
        let state = state(&["notes", "api: auth*", "api: billing", "web: ui"]);

        assert_eq!(
            shown(&widget(&[]), &state),
            " notes |api:<auth>| billing |web: ui "
        );
    }

    #[test]
    fn group_separators_are_used_next_to_groups() {
        let widget = widget(&[("tab_group_separator", " ⋮ ")]);
        let state = state(&["notes", "api: a*", "api: b", "web: x", "misc"]);

        assert_eq!(
            shown(&widget, &state),
            " notes  ⋮ api:<a>| b  ⋮ web: x  ⋮  misc "
        );
    }

    #[test]
    fn a_collapsed_group_shows_its_size_and_statuses() {
        let state = collapse(state(&["notes*", "api: a", "api: b~", "api: c~"]), "api");

        assert_eq!(shown(&widget(&[]), &state), "<notes>|api+3◐2");
    }

    #[test]
    fn a_collapsed_group_with_the_active_tab_shows_it_and_the_hidden_tabs() {
        let state = collapse(state(&["notes", "api: a~", "api: b*", "api: c"]), "api");

        assert_eq!(shown(&widget(&[]), &state), " notes |api:<b>+2◐1");
    }

    #[test]
    fn groups_can_start_collapsed() {
        let widget = widget(&[("tab_group_start_collapsed", "true")]);
        let mut state = state(&["notes*", "api: a", "api: b"]);

        assert_eq!(shown(&widget, &state), "<notes>|api+2");
        state.tab_groups.collapse = CollapseState::default();
        state.tab_groups.collapse.set_all(false);
        assert_eq!(shown(&widget, &state), "<notes>|api: a | b ");
    }

    #[test]
    fn auto_mode_expands_only_the_active_group() {
        let widget = widget(&[("tab_group_collapse", "auto")]);
        let state = collapse(state(&["api: a*", "api: b", "web: x", "web: y~"]), "api");

        assert_eq!(shown(&widget, &state), "api:<a>| b |web+2◐1");
        assert_eq!(
            click(&widget, &state, 13),
            Some(Action::Switch(3)),
            "goes to the busy web tab"
        );
        assert_eq!(
            click(&widget, &state, 0),
            None,
            "the expanded group's label does nothing"
        );
    }

    #[test]
    fn a_group_of_one_tab_shows_its_label_and_never_collapses() {
        let lone = widget(&[]);
        let tabs = collapse(state(&["notes", "solo: x", "api: a*", "api: b"]), "solo");
        // " notes |solo: x |api:<a>| b "
        assert_eq!(shown(&lone, &tabs), " notes |solo: x |api:<a>| b ");
        assert_eq!(
            click(&lone, &tabs, 8),
            Some(Action::Switch(1)),
            "its label goes to its tab"
        );
        assert_eq!(click(&lone, &tabs, 13), Some(Action::Switch(1)));

        for extra in [
            ("tab_group_start_collapsed", "true"),
            ("tab_group_collapse", "auto"),
        ] {
            let lone = widget(&[extra]);
            let tabs = state(&["solo: x", "api: a", "api: b*"]);
            let bar = shown(&lone, &tabs);
            assert!(bar.starts_with("solo: x |"), "{extra:?}: {bar}");
        }
    }

    #[test]
    fn renaming_shows_the_full_name() {
        let mut state = state(&["api: a*", "api: b"]);
        state.mode = ModeInfo {
            mode: InputMode::RenameTab,
            ..ModeInfo::default()
        };

        assert_eq!(shown(&widget(&[]), &state), "api:<api: a>| b ");
    }

    #[test]
    fn the_placeholders_work_in_tabs_too() {
        let widget = widget(&[("tab_normal", " {attention}{name} ")]);
        let state = state(&["notes*", "api: a~", "api: b", "web: x~"]);

        assert_eq!(shown(&widget, &state), "<notes>|api: ◐a | b |web: ◐x ");
    }

    #[test]
    fn clicks_on_labels_toggle_and_clicks_on_tabs_switch() {
        let widget = widget(&[]);
        let state = state(&["notes", "api: a*", "api: b"]);
        // " notes |api:<a>| b "
        assert_eq!(click(&widget, &state, 0), Some(Action::Switch(0)));
        assert_eq!(click(&widget, &state, 6), Some(Action::Switch(0)));
        assert_eq!(click(&widget, &state, 7), None, "separator");
        assert_eq!(
            click(&widget, &state, 8),
            Some(Action::Toggle("api".to_owned()))
        );
        assert_eq!(click(&widget, &state, 12), Some(Action::Switch(1)));
        assert_eq!(click(&widget, &state, 16), Some(Action::Switch(2)));
        assert_eq!(click(&widget, &state, 40), None);
    }

    #[test]
    fn clicks_on_counts_go_to_the_most_urgent_tab() {
        let widget = widget(&[]);
        let state = collapse(state(&["notes*", "api: a", "api: b~"]), "api");
        // "<notes>|api+2◐1"
        assert_eq!(
            click(&widget, &state, 8),
            Some(Action::Toggle("api".to_owned()))
        );
        assert_eq!(click(&widget, &state, 13), Some(Action::Switch(2)));
        assert_eq!(click(&widget, &state, 14), Some(Action::Switch(2)));
    }

    #[test]
    fn only_a_left_click_acts() {
        let widget = widget(&[]);
        let mut state = state(&["notes", "api: a*", "api: b"]);
        let groups = widget.groups.as_ref().unwrap();

        state.tab_groups.left_click = false;
        assert_eq!(widget.clicked(groups, &state, 8), None);
        state.tab_groups.left_click = true;
        assert_eq!(
            widget.clicked(groups, &state, 8),
            Some(Action::Toggle("api".to_owned()))
        );
    }

    #[test]
    fn truncation_windows_over_items() {
        let widget = widget(&[
            ("tab_display_count", "2"),
            ("tab_truncate_start_format", "<{count} "),
            ("tab_truncate_end_format", " {count}>"),
        ]);
        let state = state(&["one", "two", "api: a", "api: b*", "three"]);

        assert_eq!(shown(&widget, &state), "<1  two |api: a |<b> 1>");
        assert_eq!(
            click(&widget, &state, 0),
            Some(Action::Switch(0)),
            "the tab before"
        );
        assert_eq!(
            click(&widget, &state, 21),
            Some(Action::Switch(4)),
            "the tab after"
        );
    }

    #[test]
    fn windows_like_get_tab_window() {
        assert_eq!(window(3, Some(1), None), (0, 3));
        assert_eq!(window(3, Some(1), Some(3)), (0, 3));
        assert_eq!(window(5, Some(2), Some(3)), (1, 4));
        assert_eq!(window(5, Some(0), Some(3)), (0, 3));
        assert_eq!(window(5, Some(4), Some(3)), (2, 5));
        assert_eq!(window(5, None, Some(2)), (0, 2));
    }

    #[test]
    fn without_groups_the_bar_is_unchanged() {
        let mut config = BTreeMap::new();
        config.insert("tab_normal".to_owned(), " {name} ".to_owned());
        config.insert("tab_separator".to_owned(), "|".to_owned());
        let widget = TabsWidget::new(&config);

        assert!(widget.groups.is_none());
        assert_eq!(
            shown(&widget, &state(&["api: a*", "api: b"])),
            " api: a | api: b "
        );
    }
}
