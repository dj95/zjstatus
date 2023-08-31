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
        let session = state
            .sessions
            .into_iter()
            .filter(|s| s.is_current_session)
            .next();

        match session {
            Some(s) => s.name,
            None => "".to_string(),
        }
    }
}
