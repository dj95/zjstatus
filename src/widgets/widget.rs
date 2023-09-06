use crate::ZellijState;

pub trait Widget {
    fn process(&self, state: ZellijState) -> String;
    fn process_click(&self, state: ZellijState, pos: usize);
}
