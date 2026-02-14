use crate::config::ZellijState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClickType {
    Left,
    Right,
}

pub trait Widget {
    fn process(&self, name: &str, state: &ZellijState) -> String;
    fn process_click(&self, name: &str, state: &ZellijState, pos: usize, click_type: ClickType);
}
