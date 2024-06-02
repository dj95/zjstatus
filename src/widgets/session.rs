use std::collections::BTreeMap;

use crate::{config::ZellijState, widgets::widget::Widget};

pub struct SessionWidget {}

impl SessionWidget {
    pub fn new(_config: &BTreeMap<String, String>) -> Self {
        Self {}
    }
}

impl Widget for SessionWidget {
    fn process(&self, _name: &str, state: &ZellijState) -> String {
        match &state.mode.session_name {
            Some(name) => name.to_owned(),
            None => "".to_owned(),
        }
    }

    fn process_click(&self, _name: &str, _state: &ZellijState, _pos: usize) {}
}
