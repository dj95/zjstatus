use std::collections::BTreeMap;

use super::widget::Widget;

pub struct SessionWidget {}

impl SessionWidget {
    pub fn new(_config: BTreeMap<String, String>) -> Self {
        Self {}
    }
}

impl Widget for SessionWidget {
    fn process(&self, state: crate::ZellijState) -> String {
        match state.mode.session_name {
            Some(name) => name,
            None => "".to_string(),
        }
    }

    fn process_click(&self, _state: crate::ZellijState, _pos: usize) {}
}
