use std::collections::BTreeMap;

use crate::{config::ZellijState, widgets::widget::Widget};

pub struct SessionWidget {}

impl SessionWidget {
    pub fn new(_config: BTreeMap<String, String>) -> Self {
        Self {}
    }
}

impl Widget for SessionWidget {
    fn process(&self, _name: &str, state: &ZellijState) -> String {
        match &state.mode.session_name {
            Some(name) => name.to_string(),
            None => "".to_string(),
        }
    }

    fn process_click(&self, _state: &ZellijState, _pos: usize) {}
}
