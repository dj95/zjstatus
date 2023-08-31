use crate::ZellijState;

pub trait Widget {
    fn process(&self, state: ZellijState) -> String;
}
