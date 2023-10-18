use std::collections::BTreeMap;

use zellij_tile::shim::run_command;

use super::widget::Widget;

pub struct CommandWidget {}

impl CommandWidget {
    pub fn new(_config: BTreeMap<String, String>) -> Self {
        Self {}
    }
}

impl Widget for CommandWidget {
    fn process(&self, name: &str, state: crate::ZellijState) -> String {
        let mut context = BTreeMap::new();
        context.insert("foo".to_owned(), "bar".to_owned());
        run_command(&["ls", "-la"], context);

        "".to_string()
    }

    fn process_click(&self, _state: crate::ZellijState, _pos: usize) {}
}
