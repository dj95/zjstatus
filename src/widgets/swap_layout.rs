use std::collections::BTreeMap;
use zellij_tile::shim::next_swap_layout;

use crate::{config::ZellijState, widgets::widget::Widget};

pub struct SwapLayoutWidget {}

impl SwapLayoutWidget {
    pub fn new(_config: &BTreeMap<String, String>) -> Self {
        Self {}
    }
}

impl Widget for SwapLayoutWidget {
    fn process(&self, _name: &str, state: &ZellijState) -> String {
        let active_tab = state.tabs.iter().find(|t| t.active);

        if active_tab.is_none() {
            return "".to_owned();
        }

        let active_tab = active_tab.unwrap();

        match active_tab.active_swap_layout_name.clone() {
            Some(n) => n,
            None => "".to_owned(),
        }
    }

    fn process_click(&self, _state: &ZellijState, _pos: usize) {
        next_swap_layout()
    }
}
