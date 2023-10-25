use std::collections::BTreeMap;
use zellij_tile::shim::next_swap_layout;

use super::widget::Widget;

pub struct SwapLayoutWidget {}

impl SwapLayoutWidget {
    pub fn new(_config: BTreeMap<String, String>) -> Self {
        Self {}
    }
}

impl Widget for SwapLayoutWidget {
    fn process(&self, _name: &str, state: crate::ZellijState) -> String {
        let active_tab = state.tabs.iter().find(|t| t.active);

        if active_tab.is_none() {
            return "".to_string();
        }

        let active_tab = active_tab.unwrap();

        match active_tab.active_swap_layout_name.clone() {
            Some(n) => n,
            None => "".to_string(),
        }

    }

    fn process_click(&self, _state: crate::ZellijState, _pos: usize) {
        next_swap_layout()
    }
}
