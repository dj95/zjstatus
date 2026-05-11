use crate::config::ZellijState;

pub trait Widget {
    fn process(&self, name: &str, state: &ZellijState) -> String;
    fn process_click(&self, name: &str, state: &ZellijState, pos: usize);
    fn process_scroll(&self, _name: &str, _state: &mut ZellijState, _delta: isize) -> bool {
        false
    }
    fn is_truncatable(&self, _name: &str) -> bool {
        false
    }
    fn truncate(
        &self,
        _name: &str,
        output: &str,
        max_width: usize,
        _state: &ZellijState,
    ) -> String {
        crate::render::truncate_ansi_string_to_width_from(output, "...", max_width, 0)
    }
}
