use std::collections::BTreeMap;

use zellij_tile::prelude::InputMode;

use crate::{render::FormattedPart, config::ZellijState};

use super::widget::Widget;

#[derive(Debug)]
pub struct ModeWidget {
    normal_format: Vec<FormattedPart>,
    locked_format: Vec<FormattedPart>,
    resize_format: Vec<FormattedPart>,
    pane_format: Vec<FormattedPart>,
    tab_format: Vec<FormattedPart>,
    scroll_format: Vec<FormattedPart>,
    enter_search_format: Vec<FormattedPart>,
    search_format: Vec<FormattedPart>,
    rename_tab_format: Vec<FormattedPart>,
    rename_pane_format: Vec<FormattedPart>,
    session_format: Vec<FormattedPart>,
    move_format: Vec<FormattedPart>,
    prompt_format: Vec<FormattedPart>,
    tmux_format: Vec<FormattedPart>,
}

impl ModeWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let normal_format = match config.get("mode_normal") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => vec![],
        };

        let locked_format = match config.get("mode_locked") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let resize_format = match config.get("mode_resize") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let pane_format = match config.get("mode_pane") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let tab_format = match config.get("mode_tab") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let scroll_format = match config.get("mode_scroll") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let enter_search_format = match config.get("mode_enter_search") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let search_format = match config.get("mode_search") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let rename_tab_format = match config.get("mode_rename_tab") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let rename_pane_format = match config.get("mode_rename_pane") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let session_format = match config.get("mode_session") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let move_format = match config.get("mode_move") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let prompt_format = match config.get("mode_prompt") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        let tmux_format = match config.get("mode_tmux") {
            Some(form) => FormattedPart::multiple_from_format_string(form),
            None => normal_format.clone(),
        };

        Self {
            normal_format,
            locked_format,
            resize_format,
            pane_format,
            tab_format,
            scroll_format,
            enter_search_format,
            search_format,
            rename_tab_format,
            rename_pane_format,
            session_format,
            move_format,
            prompt_format,
            tmux_format,
        }
    }
}

impl Widget for ModeWidget {
    fn process(&self, _name: &str, state: &ZellijState) -> String {
        self.select_format(state.mode.mode)
            .iter()
            .map(|f| {
                let mut content = f.content.clone();

                if f.content.contains("{name}") {
                    content = f
                        .content
                        .replace("{name}", format!("{:?}", state.mode.mode).as_str());
                }

                (f, content)
            })
            .fold("".to_owned(), |acc, (f, content)| {
                format!("{acc}{}", f.format_string(&content))
            })
    }

    fn process_click(&self, _state: &ZellijState, _pos: usize) {}
}

impl ModeWidget {
    fn select_format(&self, mode: InputMode) -> &Vec<FormattedPart> {
        match mode {
            InputMode::Normal => &self.normal_format,
            InputMode::Locked => &self.locked_format,
            InputMode::Resize => &self.resize_format,
            InputMode::Pane => &self.pane_format,
            InputMode::Tab => &self.tab_format,
            InputMode::Scroll => &self.scroll_format,
            InputMode::EnterSearch => &self.enter_search_format,
            InputMode::Search => &self.search_format,
            InputMode::RenameTab => &self.rename_tab_format,
            InputMode::RenamePane => &self.rename_pane_format,
            InputMode::Session => &self.session_format,
            InputMode::Move => &self.move_format,
            InputMode::Prompt => &self.prompt_format,
            InputMode::Tmux => &self.tmux_format,
        }
    }
}
