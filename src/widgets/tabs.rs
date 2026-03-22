use std::{cmp, collections::BTreeMap};

use zellij_tile::{
    prelude::{InputMode, ModeInfo, PaneInfo, PaneManifest, TabInfo},
    shim::switch_tab_to,
};

use crate::{config::ZellijState, render::FormattedPart};

use super::widget::Widget;

struct TabWindow {
    truncated_start: usize,
    truncated_end: usize,
    tabs: Vec<TabInfo>,
    /// Partially visible tab at the left edge, with max display width.
    left_boundary: Option<(TabInfo, usize)>,
    /// Partially visible tab at the right edge, with max display width.
    right_boundary: Option<(TabInfo, usize)>,
    /// Max total output width for the tab section (0 = no limit).
    available: usize,
}

pub struct TabsWidget {
    active_tab_format: Vec<FormattedPart>,
    active_tab_fullscreen_format: Vec<FormattedPart>,
    active_tab_sync_format: Vec<FormattedPart>,
    normal_tab_format: Vec<FormattedPart>,
    normal_tab_fullscreen_format: Vec<FormattedPart>,
    normal_tab_sync_format: Vec<FormattedPart>,
    rename_tab_format: Vec<FormattedPart>,
    separator: Option<FormattedPart>,
    fullscreen_indicator: Option<String>,
    floating_indicator: Option<String>,
    sync_indicator: Option<String>,
    tab_display_count: Option<usize>,
    tab_truncate_start_format: Vec<FormattedPart>,
    tab_truncate_end_format: Vec<FormattedPart>,
}

impl TabsWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let mut normal_tab_format: Vec<FormattedPart> = Vec::new();
        if let Some(form) = config.get("tab_normal") {
            normal_tab_format = FormattedPart::multiple_from_format_string(form, config);
        }

        let normal_tab_fullscreen_format = match config.get("tab_normal_fullscreen") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => normal_tab_format.clone(),
        };

        let normal_tab_sync_format = match config.get("tab_normal_sync") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => normal_tab_format.clone(),
        };

        let mut active_tab_format = normal_tab_format.clone();
        if let Some(form) = config.get("tab_active") {
            active_tab_format = FormattedPart::multiple_from_format_string(form, config);
        }

        let active_tab_fullscreen_format = match config.get("tab_active_fullscreen") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => active_tab_format.clone(),
        };

        let active_tab_sync_format = match config.get("tab_active_sync") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => active_tab_format.clone(),
        };

        let rename_tab_format = match config.get("tab_rename") {
            Some(form) => FormattedPart::multiple_from_format_string(form, config),
            None => active_tab_format.clone(),
        };

        let tab_display_count = match config.get("tab_display_count") {
            Some(count) => count.parse::<usize>().ok(),
            None => None,
        };

        let tab_truncate_start_format = config
            .get("tab_truncate_start_format")
            .map(|form| FormattedPart::multiple_from_format_string(form, config))
            .unwrap_or_default();

        let tab_truncate_end_format = config
            .get("tab_truncate_end_format")
            .map(|form| FormattedPart::multiple_from_format_string(form, config))
            .unwrap_or_default();

        let separator = config
            .get("tab_separator")
            .map(|s| FormattedPart::from_format_string(s, config));

        Self {
            normal_tab_format,
            normal_tab_fullscreen_format,
            normal_tab_sync_format,
            active_tab_format,
            active_tab_fullscreen_format,
            active_tab_sync_format,
            rename_tab_format,
            separator,
            floating_indicator: config.get("tab_floating_indicator").cloned(),
            sync_indicator: config.get("tab_sync_indicator").cloned(),
            fullscreen_indicator: config.get("tab_fullscreen_indicator").cloned(),
            tab_display_count,
            tab_truncate_start_format,
            tab_truncate_end_format,
        }
    }
}

impl Widget for TabsWidget {
    fn process(&self, _name: &str, state: &ZellijState) -> String {
        let window = self.visible_tab_window(state);
        let mut output = "".to_owned();

        if window.truncated_start > 0 {
            for f in &self.tab_truncate_start_format {
                let mut content = f.content.clone();

                if content.contains("{count}") {
                    content =
                        content.replace("{count}", (window.truncated_start).to_string().as_str());
                }

                output = format!("{output}{}", f.format_string(&content));
            }
        }

        if let Some((ref tab, max_width)) = window.left_boundary {
            let rendered = self.render_tab(tab, &state.panes, &state.mode);
            output = format!(
                "{output}{}",
                console::truncate_str(&rendered, max_width, "\u{2026}")
            );
            if let Some(sep) = &self.separator {
                output = format!("{output}{}", sep.format_string(&sep.content));
            }
        }

        let tab_count = window.tabs.len();
        for (i, tab) in window.tabs.iter().enumerate() {
            let content = self.render_tab(tab, &state.panes, &state.mode);

            output = format!("{}{}", output, content);

            if (i + 1 < tab_count || window.right_boundary.is_some())
                && let Some(sep) = &self.separator
            {
                output = format!("{}{}", output, sep.format_string(&sep.content));
            }
        }

        if let Some((ref tab, max_width)) = window.right_boundary {
            let rendered = self.render_tab(tab, &state.panes, &state.mode);
            output = format!(
                "{output}{}",
                console::truncate_str(&rendered, max_width, "\u{2026}")
            );
        }

        if window.truncated_end > 0 {
            for f in &self.tab_truncate_end_format {
                let mut content = f.content.clone();

                if content.contains("{count}") {
                    content =
                        content.replace("{count}", (window.truncated_end).to_string().as_str());
                }

                output = format!("{output}{}", f.format_string(&content));
            }
        }

        if window.available > 0 && console::measure_text_width(&output) > window.available {
            output = console::truncate_str(&output, window.available, "\u{2026}").to_string();
        }

        output
    }

    fn process_click(&self, _name: &str, state: &ZellijState, pos: usize) {
        let mut offset = 0;
        let window = self.visible_tab_window(state);

        let active_pos = &state
            .tabs
            .iter()
            .find(|t| t.active)
            .expect("no active tab")
            .position
            + 1;

        if window.truncated_start > 0 {
            for f in &self.tab_truncate_start_format {
                let mut content = f.content.clone();

                if content.contains("{count}") {
                    content =
                        content.replace("{count}", (window.truncated_start).to_string().as_str());
                }

                offset += console::measure_text_width(&f.format_string(&content));

                if pos <= offset {
                    switch_tab_to(active_pos.saturating_sub(1) as u32);
                    return;
                }
            }
        }

        if let Some((ref tab, max_width)) = window.left_boundary {
            let rendered = self.render_tab(tab, &state.panes, &state.mode);
            let truncated = console::truncate_str(&rendered, max_width, "\u{2026}");
            let mut content = truncated.to_string();
            if let Some(sep) = &self.separator {
                content = format!("{}{}", content, sep.format_string(&sep.content));
            }
            let content_len = console::measure_text_width(&content);
            if pos > offset && pos < offset + content_len {
                switch_tab_to(tab.position as u32 + 1);
                return;
            }
            offset += content_len;
        }

        let tab_count = window.tabs.len();
        for (i, tab) in window.tabs.iter().enumerate() {
            let mut rendered_content = self.render_tab(tab, &state.panes, &state.mode);

            if (i + 1 < tab_count || window.right_boundary.is_some())
                && let Some(sep) = &self.separator
            {
                rendered_content =
                    format!("{}{}", rendered_content, sep.format_string(&sep.content));
            }

            let content_len = console::measure_text_width(&rendered_content);

            if pos > offset && pos < offset + content_len {
                switch_tab_to(tab.position as u32 + 1);

                break;
            }

            offset += content_len;
        }

        if let Some((ref tab, max_width)) = window.right_boundary {
            let rendered = self.render_tab(tab, &state.panes, &state.mode);
            let truncated = console::truncate_str(&rendered, max_width, "\u{2026}");
            let content_len = console::measure_text_width(&truncated);
            if pos > offset && pos < offset + content_len {
                switch_tab_to(tab.position as u32 + 1);
                return;
            }
            offset += content_len;
        }

        if window.truncated_end > 0 {
            for f in &self.tab_truncate_end_format {
                let mut content = f.content.clone();

                if content.contains("{count}") {
                    content =
                        content.replace("{count}", (window.truncated_end).to_string().as_str());
                }

                offset += console::measure_text_width(&f.format_string(&content));

                if pos <= offset {
                    switch_tab_to(cmp::min(active_pos + 1, state.tabs.len()) as u32);
                    return;
                }
            }
        }
    }
}

impl TabsWidget {
    /// Get the visible tab window. Uses fixed count windowing if tab_display_count
    /// is configured, otherwise fits tabs to the available width.
    fn visible_tab_window(&self, state: &ZellijState) -> TabWindow {
        if self.tab_display_count.is_some() {
            let (start, end, tabs) = get_tab_window(&state.tabs, self.tab_display_count);
            return TabWindow {
                truncated_start: start,
                truncated_end: end,
                tabs,
                left_boundary: None,
                right_boundary: None,
                available: 0,
            };
        }
        if state.cols > 0 {
            let available = state.cols.saturating_sub(state.reserved_cols);
            return self.fit_tab_window(&state.tabs, &state.panes, &state.mode, available);
        }
        TabWindow {
            truncated_start: 0,
            truncated_end: 0,
            tabs: state.tabs.to_vec(),
            left_boundary: None,
            right_boundary: None,
            available: 0,
        }
    }

    /// Find the widest window of tabs around the active tab that fits within
    /// the given column budget. Boundary tabs that don't fully fit are included
    /// with a max display width for truncated rendering.
    fn fit_tab_window(
        &self,
        tabs: &[TabInfo],
        panes: &PaneManifest,
        mode: &ModeInfo,
        available: usize,
    ) -> TabWindow {
        if tabs.is_empty() {
            return TabWindow {
                truncated_start: 0,
                truncated_end: 0,
                tabs: vec![],
                left_boundary: None,
                right_boundary: None,
                available,
            };
        }

        let max_count_str = tabs.len().to_string();
        let trunc_end_width: usize = self.tab_truncate_end_format.iter().map(|f| {
            let content = f.content.replace("{count}", &max_count_str);
            console::measure_text_width(&f.format_string(&content))
        }).sum();
        let trunc_start_width: usize = self.tab_truncate_start_format.iter().map(|f| {
            let content = f.content.replace("{count}", &max_count_str);
            console::measure_text_width(&f.format_string(&content))
        }).sum();

        let sep_width = self.separator.as_ref()
            .map(|s| console::measure_text_width(&s.format_string(&s.content)))
            .unwrap_or(0);

        let tab_widths: Vec<usize> = tabs.iter()
            .map(|t| console::measure_text_width(&self.render_tab(t, panes, mode)))
            .collect();

        let total: usize = tab_widths.iter().sum::<usize>()
            + sep_width * tabs.len().saturating_sub(1);
        if total <= available {
            return TabWindow {
                truncated_start: 0,
                truncated_end: 0,
                tabs: tabs.to_vec(),
                left_boundary: None,
                right_boundary: None,
                available,
            };
        }

        let active_idx = tabs.iter().position(|t| t.active).unwrap_or(0);
        let mut left = active_idx;
        let mut right = active_idx;
        let mut width = tab_widths[active_idx];

        loop {
            let can_go_left = left > 0;
            let can_go_right = right + 1 < tabs.len();

            if !can_go_left && !can_go_right {
                break;
            }

            // Reserve space for truncation indicators after expanding right.
            let trunc_reserve = if left > 0 { trunc_start_width } else { 0 }
                + if right + 2 < tabs.len() { trunc_end_width } else { 0 };

            let mut grew = false;
            if can_go_right {
                let candidate = width + sep_width + tab_widths[right + 1];
                if candidate + trunc_reserve <= available {
                    right += 1;
                    width = candidate;
                    grew = true;
                }
            }
            if can_go_left {
                let new_trunc_reserve = if left > 1 { trunc_start_width } else { 0 }
                    + if right + 1 < tabs.len() { trunc_end_width } else { 0 };
                let candidate = width + sep_width + tab_widths[left - 1];
                if candidate + new_trunc_reserve <= available {
                    left -= 1;
                    width = candidate;
                    grew = true;
                }
            }

            if !grew {
                break;
            }
        }

        // Try to partially show boundary tabs in remaining space.
        let start_trunc = if left > 0 { trunc_start_width } else { 0 };
        let end_trunc = if right + 1 < tabs.len() { trunc_end_width } else { 0 };
        let mut remaining = available.saturating_sub(width + start_trunc + end_trunc);

        // Only show boundary tabs when 2+ tabs are hidden on that side,
        // so the truncation indicator (with decremented count) always remains.
        let mut right_boundary = None;
        if right + 2 < tabs.len() {
            let min_width = trunc_end_width.max(1);
            let budget = remaining.saturating_sub(sep_width);
            if budget >= min_width {
                right_boundary = Some((tabs[right + 1].clone(), budget));
                remaining = remaining.saturating_sub(sep_width + budget);
            }
        }

        let mut left_boundary = None;
        if left > 1 {
            let min_width = trunc_start_width.max(1);
            let budget = remaining.saturating_sub(sep_width);
            if budget >= min_width {
                left_boundary = Some((tabs[left - 1].clone(), budget));
            }
        }

        let truncated_start = if left_boundary.is_some() { left - 1 } else { left };
        let truncated_end = if right_boundary.is_some() {
            tabs.len() - right - 2
        } else {
            tabs.len() - right - 1
        };

        TabWindow {
            truncated_start,
            truncated_end,
            tabs: tabs[left..=right].to_vec(),
            left_boundary,
            right_boundary,
            available,
        }
    }

    fn select_format(&self, info: &TabInfo, mode: &ModeInfo) -> &Vec<FormattedPart> {
        if info.active && mode.mode == InputMode::RenameTab {
            return &self.rename_tab_format;
        }

        if info.active && info.is_fullscreen_active {
            return &self.active_tab_fullscreen_format;
        }

        if info.active && info.is_sync_panes_active {
            return &self.active_tab_sync_format;
        }

        if info.active {
            return &self.active_tab_format;
        }

        if info.is_fullscreen_active {
            return &self.normal_tab_fullscreen_format;
        }

        if info.is_sync_panes_active {
            return &self.normal_tab_sync_format;
        }

        &self.normal_tab_format
    }

    fn render_tab(&self, tab: &TabInfo, panes: &PaneManifest, mode: &ModeInfo) -> String {
        let formatters = self.select_format(tab, mode);
        let mut output = "".to_owned();

        for f in formatters.iter() {
            let mut content = f.content.clone();

            let tab_name = match mode.mode {
                InputMode::RenameTab => match tab.name.is_empty() {
                    true => "Enter name...",
                    false => tab.name.as_str(),
                },
                _name => tab.name.as_str(),
            };

            if content.contains("{name}") {
                content = content.replace("{name}", tab_name);
            }

            if content.contains("{index}") {
                content = content.replace("{index}", (tab.position + 1).to_string().as_str());
            }

            if content.contains("{floating_total_count}") {
                let panes_for_tab: Vec<PaneInfo> =
                    panes.panes.get(&tab.position).cloned().unwrap_or_default();

                content = content.replace(
                    "{floating_total_count}",
                    &format!("{}", panes_for_tab.iter().filter(|p| p.is_floating).count()),
                );
            }

            content = self.replace_indicators(content, tab, panes);

            output = format!("{}{}", output, f.format_string(&content));
        }

        output.to_owned()
    }

    fn replace_indicators(&self, content: String, tab: &TabInfo, panes: &PaneManifest) -> String {
        let mut content = content;
        if content.contains("{fullscreen_indicator}") && self.fullscreen_indicator.is_some() {
            content = content.replace(
                "{fullscreen_indicator}",
                if tab.is_fullscreen_active {
                    self.fullscreen_indicator.as_ref().unwrap()
                } else {
                    ""
                },
            );
        }

        if content.contains("{sync_indicator}") && self.sync_indicator.is_some() {
            content = content.replace(
                "{sync_indicator}",
                if tab.is_sync_panes_active {
                    self.sync_indicator.as_ref().unwrap()
                } else {
                    ""
                },
            );
        }

        if content.contains("{floating_indicator}") && self.floating_indicator.is_some() {
            let panes_for_tab: Vec<PaneInfo> =
                panes.panes.get(&tab.position).cloned().unwrap_or_default();

            let is_floating = panes_for_tab.iter().any(|p| p.is_floating);

            content = content.replace(
                "{floating_indicator}",
                if is_floating {
                    self.floating_indicator.as_ref().unwrap()
                } else {
                    ""
                },
            );
        }

        content
    }
}

pub fn get_tab_window(
    tabs: &Vec<TabInfo>,
    max_count: Option<usize>,
) -> (usize, usize, Vec<TabInfo>) {
    let max_count = match max_count {
        Some(count) => count,
        None => return (0, 0, tabs.to_vec()),
    };

    if tabs.len() <= max_count {
        return (0, 0, tabs.to_vec());
    }

    let active_index = tabs.iter().position(|t| t.active).expect("no active tab");

    // active tab is in the last #max_count tabs, so return the last #max_count
    if active_index > tabs.len().saturating_sub(max_count) {
        return (
            tabs.len().saturating_sub(max_count),
            0,
            tabs.iter()
                .cloned()
                .rev()
                .take(max_count)
                .rev()
                .collect::<Vec<TabInfo>>(),
        );
    }

    // tabs must be truncated
    let first_index = active_index.saturating_sub(1);
    let last_index = cmp::min(first_index + max_count, tabs.len());

    (
        first_index,
        tabs.len().saturating_sub(last_index),
        tabs.as_slice()[first_index..last_index].to_vec(),
    )
}

#[cfg(test)]
mod test {
    use zellij_tile::prelude::TabInfo;

    use super::get_tab_window;
    use rstest::rstest;

    #[rstest]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (1, 1, vec![
                TabInfo {
                    active: false,
                    name: "2".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "4".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: true,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 2, vec![
                TabInfo {
                    active: true,
                    name: "1".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "2".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 2, vec![
                TabInfo {
                    active: false,
                    name: "1".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "2".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (2, 0, vec![
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "4".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "5".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (2, 0, vec![
                TabInfo {
                    active: false,
                    name: "3".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: true,
                    name: "4".to_owned(),
                    ..TabInfo::default()
                },
                TabInfo {
                    active: false,
                    name: "5".to_owned(),
                    ..TabInfo::default()
                },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
        ],
        None,
        (0, 0, vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "4".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "5".to_owned(),
                ..TabInfo::default()
            },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 0, vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            ]
        )
    )]
    #[case(
        vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
        ],
        Some(3),
        (0, 0, vec![
            TabInfo {
                active: false,
                name: "1".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: true,
                name: "2".to_owned(),
                ..TabInfo::default()
            },
            TabInfo {
                active: false,
                name: "3".to_owned(),
                ..TabInfo::default()
            },
            ]
        )
    )]
    pub fn test_get_tab_window(
        #[case] tabs: Vec<TabInfo>,
        #[case] max_count: Option<usize>,
        #[case] expected: (usize, usize, Vec<TabInfo>),
    ) {
        let res = get_tab_window(&tabs, max_count);

        assert_eq!(res, expected);
    }
}
