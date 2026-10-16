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
    pub tab_name_max_width: Option<usize>,
}

#[derive(Clone, Debug, Ord, Eq, PartialEq, PartialOrd, Copy)]
pub enum Part {
    Left,
    Center,
    Right,
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
    pub pane_frame_style: String,
    pub border: BorderConfig,
    pub format_precedence: Vec<Part>,
    pub hide_on_overlength: bool,
    pub tab_name_shrink_on_overlength: bool,
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
        let pane_frame_style = match config.get("pane_frame_style") {
            Some(style) => style.to_owned(),
            None => "titles".to_owned(),
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

        let tab_name_shrink_on_overlength = match config.get("tab_name_shrink_on_overlength") {
            Some(opt) => opt == "true",
            None => false,
        };

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
            pane_frame_style,
            border: border_config,
            format_precedence,
            hide_on_overlength,
            tab_name_shrink_on_overlength,
        })
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
            Mouse::ScrollLeft(_) => return,
            Mouse::ScrollRight(_) => return,
            Mouse::LeftClick(_, y) => y,
            Mouse::RightClick(_, y) => y,
            Mouse::Hold(_, y) => y,
            Mouse::Release(_, y) => y,
            Mouse::Hover(_, _) => return,
        };

        let mut state = state;
        let (output_left, output_center, output_right) = self.render_parts(&mut state, &widget_map);

        let (output_left, output_center, output_right) = match self.hide_on_overlength {
            true => self.trim_output(&output_left, &output_center, &output_right, state.cols),
            false => (output_left, output_center, output_right),
        };

        let mut offset = console::measure_text_width(&output_left);

        self.process_widget_click(click_pos, &self.left_parts, &widget_map, &state, 0);

        if click_pos <= offset {
            return;
        }

        if !output_center.is_empty() {
            tracing::debug!("widgetclick center");
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

    pub fn render_bar(
        &mut self,
        state: ZellijState,
        widget_map: BTreeMap<String, Arc<dyn Widget>>,
    ) -> String {
        if self.left_parts.is_empty() && self.center_parts.is_empty() && self.right_parts.is_empty()
        {
            return "No configuration found. See https://github.com/dj95/zjstatus/wiki/3-%E2%80%90-Configuration for more info".to_string();
        }

        let mut state = state;
        let (output_left, output_center, output_right) = self.render_parts(&mut state, &widget_map);

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
        &mut self,
        state: &mut ZellijState,
        widget_map: &BTreeMap<String, Arc<dyn Widget>>,
    ) -> (String, String, String) {
        let render = |parts: &mut Vec<FormattedPart>, state: &ZellijState| {
            parts.iter_mut().fold("".to_owned(), |acc, part| {
                format!(
                    "{acc}{}",
                    part.format_string_with_widgets(widget_map, state)
                )
            })
        };

        let tabs_part = [Part::Left, Part::Center, Part::Right]
            .into_iter()
            .find(|p| {
                self.parts_mut(*p)
                    .iter()
                    .any(|f| f.content.contains("{tabs}"))
            });

        let shrink = match (
            self.tab_name_shrink_on_overlength,
            tabs_part,
            widget_map.get("tabs"),
        ) {
            (true, Some(part), Some(widget)) => Some((part, widget)),
            _ => None,
        };

        if shrink.is_some() {
            // Bypass the widget cache, it may hold the names shrunk for a previous width.
            state.cache_mask |= UpdateEventMask::Tab as u8;
        }

        let mut output = BTreeMap::from([
            (Part::Left, render(&mut self.left_parts, state)),
            (Part::Center, render(&mut self.center_parts, state)),
            (Part::Right, render(&mut self.right_parts, state)),
        ]);

        if let Some((tabs_part, tabs_widget)) = shrink {
            // Only the tabs widget is re-rendered while searching the width. The part itself is
            // rendered once, other widgets in it (e.g. commands) must not run again.
            let width = console::measure_text_width;
            let full_tabs = tabs_widget.process("tabs", state);
            let rest_width = width(&output[&tabs_part]).saturating_sub(width(&full_tabs));
            let mut tabs = full_tabs.clone();

            let mut max = state.tabs.iter().map(|t| width(&t.name)).max().unwrap_or(0);

            while max > 1 && tabs_overlap(tabs_part, rest_width + width(&tabs), &output, state.cols)
            {
                max -= 1;
                state.tab_name_max_width = Some(max);
                tabs = tabs_widget.process("tabs", state);
            }

            if tabs_overlap(tabs_part, rest_width + width(&tabs), &output, state.cols) {
                // Shrinking did not resolve the overlap, keep the names readable and
                // leave hiding to trim_output.
                state.tab_name_max_width = None;
            } else if state.tab_name_max_width.is_some() && !full_tabs.is_empty() {
                // The part embeds the widget output verbatim, so swap the shrunk tabs in place.
                let part = output[&tabs_part].replace(&full_tabs, &tabs);
                output.insert(tabs_part, part);
            }
        }

        output.into_values().collect_tuple().unwrap()
    }

    fn parts_mut(&mut self, part: Part) -> &mut Vec<FormattedPart> {
        match part {
            Part::Left => &mut self.left_parts,
            Part::Center => &mut self.center_parts,
            Part::Right => &mut self.right_parts,
        }
    }

    fn trim_output(
        &self,
        output_left: &str,
        output_center: &str,
        output_right: &str,
        cols: usize,
    ) -> (String, String, String) {
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

            if parts_overlap(*a, a_count, *b, b_count, cols) {
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

fn parts_overlap(a: Part, a_count: usize, b: Part, b_count: usize, cols: usize) -> bool {
    let center_pos = (cols as f32 / 2.0).floor() as usize;

    match (a, b) {
        (Part::Left, Part::Right) | (Part::Right, Part::Left) => a_count + b_count > cols,
        (Part::Left | Part::Right, Part::Center) => {
            a_count > center_pos.saturating_sub(b_count / 2)
        }
        (Part::Center, Part::Left | Part::Right) => {
            b_count > center_pos.saturating_sub(a_count / 2)
        }
        _ => false,
    }
}

// Whether the part holding the tabs overlaps any other visible part.
fn tabs_overlap(
    tabs_part: Part,
    tabs_count: usize,
    output: &BTreeMap<Part, String>,
    cols: usize,
) -> bool {
    output.iter().any(|(part, content)| {
        let count = console::measure_text_width(content);
        *part != tabs_part && count > 0 && parts_overlap(tabs_part, tabs_count, *part, count, cols)
    })
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
    use anstyle::{Effects, RgbColor};
    use rstest::rstest;

    // Stands in for the tabs widget, whose click handler needs the zellij host.
    struct TabNamesWidget;

    impl Widget for TabNamesWidget {
        fn process(&self, _name: &str, state: &ZellijState) -> String {
            let max = state.tab_name_max_width.unwrap_or(usize::MAX);
            state
                .tabs
                .iter()
                .map(|t| format!("<{}>", console::truncate_str(&t.name, max, "…")))
                .collect()
        }

        fn process_click(&self, _name: &str, _state: &ZellijState, _pos: usize) {}
    }

    // Counts how often it is rendered, like a command widget that runs a process.
    struct CountingWidget(Arc<std::sync::atomic::AtomicUsize>);

    impl Widget for CountingWidget {
        fn process(&self, _name: &str, _state: &ZellijState) -> String {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            "".to_owned()
        }

        fn process_click(&self, _name: &str, _state: &ZellijState, _pos: usize) {}
    }

    fn shrink_module(left: &str, right: &str) -> ModuleConfig {
        ModuleConfig::new(&BTreeMap::from([
            ("format_left".to_owned(), left.to_owned()),
            ("format_right".to_owned(), right.to_owned()),
            ("format_hide_on_overlength".to_owned(), "true".to_owned()),
            (
                "tab_name_shrink_on_overlength".to_owned(),
                "true".to_owned(),
            ),
        ]))
        .unwrap()
    }

    fn shrink_state(cols: usize) -> ZellijState {
        let tab = |position: usize, name: &str| TabInfo {
            position,
            name: name.to_owned(),
            ..Default::default()
        };
        ZellijState {
            cols,
            tabs: vec![tab(0, "abcdefgh"), tab(1, "xy")],
            ..Default::default()
        }
    }

    #[test]
    fn test_render_bar_tab_name_shrink_keeps_names_when_overlap_remains() {
        let widget_map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "tabs".to_owned(),
            Arc::new(TabNamesWidget) as Arc<dyn Widget>,
        )]);
        let mut module = shrink_module("{tabs}", &"R".repeat(30));

        let out = module.render_bar(shrink_state(20), widget_map);

        assert_eq!(console::strip_ansi_codes(&out).trim_end(), "<abcdefgh><xy>");
    }

    #[test]
    fn test_render_bar_tab_name_shrink_renders_part_once() {
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let widget_map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([
            (
                "tabs".to_owned(),
                Arc::new(TabNamesWidget) as Arc<dyn Widget>,
            ),
            (
                "command".to_owned(),
                Arc::new(CountingWidget(count.clone())) as Arc<dyn Widget>,
            ),
        ]);
        let mut module = shrink_module("{tabs}{command_x}", "RIGHT");

        module.render_bar(shrink_state(14), widget_map);

        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn test_render_bar_tab_name_shrink_restores_names_on_wider_frame() {
        let widget_map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "tabs".to_owned(),
            Arc::new(TabNamesWidget) as Arc<dyn Widget>,
        )]);
        let mut module = shrink_module("{tabs}", "RIGHT");

        module.render_bar(shrink_state(14), widget_map.clone());
        let out = module.render_bar(shrink_state(40), widget_map);

        assert_eq!(
            console::strip_ansi_codes(&out).trim_end(),
            format!("<abcdefgh><xy>{}RIGHT", " ".repeat(21))
        );
    }

    #[rstest]
    #[case::tabs_left_shrink("{tabs}", "", "RIGHT", "true", 14, "<ab…><xy>RIGHT")]
    #[case::tabs_center_shrink("LEFT", "{tabs}", "", "true", 20, "LEFT<abcdef…><xy>   ")]
    #[case::disabled("{tabs}", "", "RIGHT", "false", 14, "<abcdefgh><xy>")]
    fn test_render_bar_tab_name_shrink(
        #[case] left: &str,
        #[case] center: &str,
        #[case] right: &str,
        #[case] shrink: &str,
        #[case] cols: usize,
        #[case] expected: &str,
    ) {
        let config = BTreeMap::from([
            ("format_left".to_owned(), left.to_owned()),
            ("format_center".to_owned(), center.to_owned()),
            ("format_right".to_owned(), right.to_owned()),
            ("format_hide_on_overlength".to_owned(), "true".to_owned()),
            (
                "tab_name_shrink_on_overlength".to_owned(),
                shrink.to_owned(),
            ),
        ]);
        let widget_map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([(
            "tabs".to_owned(),
            Arc::new(TabNamesWidget) as Arc<dyn Widget>,
        )]);
        let tab = |position: usize, name: &str| TabInfo {
            position,
            name: name.to_owned(),
            ..Default::default()
        };
        let state = ZellijState {
            cols,
            tabs: vec![tab(0, "abcdefgh"), tab(1, "xy")],
            ..Default::default()
        };

        let mut module = ModuleConfig::new(&config).unwrap();
        let out = module.render_bar(state, widget_map);

        assert_eq!(
            console::strip_ansi_codes(&out).trim_end(),
            expected.trim_end()
        );
    }

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
}
