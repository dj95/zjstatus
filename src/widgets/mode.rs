use std::collections::BTreeMap;

use zellij_tile::prelude::InputMode;

use crate::{render::FormattedPart, ZellijState};

use super::widget::Widget;

#[derive(Debug)]
pub struct ModeWidget {
    normal_format: FormattedPart,
    locked_format: FormattedPart,
    resize_format: FormattedPart,
    pane_format: FormattedPart,
    tab_format: FormattedPart,
    scroll_format: FormattedPart,
    enter_search_format: FormattedPart,
    search_format: FormattedPart,
    rename_tab_format: FormattedPart,
    rename_pane_format: FormattedPart,
    session_format: FormattedPart,
    move_format: FormattedPart,
    prompt_format: FormattedPart,
    tmux_format: FormattedPart,
}

impl ModeWidget {
    pub fn new(config: BTreeMap<String, String>) -> Self {
        let mut normal_format_string = "";
        if let Some(form) = config.get("mode_normal") {
            normal_format_string = form;
        }

        let locked_format = FormattedPart::from_format_string(match config.get("mode_locked") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let resize_format = FormattedPart::from_format_string(match config.get("mode_resize") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let pane_format = FormattedPart::from_format_string(match config.get("mode_pane") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let tab_format = FormattedPart::from_format_string(match config.get("mode_tab") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let scroll_format = FormattedPart::from_format_string(match config.get("mode_scroll") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let enter_search_format = FormattedPart::from_format_string(match config.get("mode_enter_search") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let search_format = FormattedPart::from_format_string(match config.get("mode_search") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let rename_tab_format = FormattedPart::from_format_string(match config.get("mode_rename_tab") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let rename_pane_format = FormattedPart::from_format_string(match config.get("mode_rename_pane") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let session_format = FormattedPart::from_format_string(match config.get("mode_session") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let move_format = FormattedPart::from_format_string(match config.get("mode_move") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let prompt_format = FormattedPart::from_format_string(match config.get("mode_prompt") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });

        let tmux_format = FormattedPart::from_format_string(match config.get("mode_tmux") {
            Some(form) => form.to_string(),
            None => normal_format_string.to_string(),
        });


        Self {
            normal_format: FormattedPart::from_format_string(normal_format_string.to_string()),
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
    fn process(&self, state: ZellijState) -> String {
        let format = self.select_format(state.mode.mode);

        let mut content = format.content.clone();
        if content.contains("{name}") {
            content = content.replace("{name}", format!("{:?}", state.mode.mode).as_str());
        }

        format.format_string(content)
    }
}

impl ModeWidget {
    fn select_format(&self, mode: InputMode) -> FormattedPart {
        match mode {
            InputMode::Normal => self.normal_format.clone(),
            InputMode::Locked => self.locked_format.clone(),
            InputMode::Resize => self.resize_format.clone(),
            InputMode::Pane => self.pane_format.clone(),
            InputMode::Tab => self.tab_format.clone(),
            InputMode::Scroll => self.scroll_format.clone(),
            InputMode::EnterSearch => self.enter_search_format.clone(),
            InputMode::Search => self.search_format.clone(),
            InputMode::RenameTab => self.rename_tab_format.clone(),
            InputMode::RenamePane => self.rename_pane_format.clone(),
            InputMode::Session => self.session_format.clone(),
            InputMode::Move => self.move_format.clone(),
            InputMode::Prompt => self.prompt_format.clone(),
            InputMode::Tmux => self.tmux_format.clone(),
        }
    }
}
