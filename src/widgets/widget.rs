use crate::config::ZellijState;

pub trait Widget {
    fn process(&self, name: &str, state: &ZellijState) -> String;
    fn process_click(&self, state: &ZellijState, pos: usize);
}
