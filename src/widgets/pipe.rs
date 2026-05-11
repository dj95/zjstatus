use std::collections::BTreeMap;

use crate::render::{
    FormattedPart, formatted_parts_from_string_cached, truncate_ansi_string_to_width_from,
};

use super::widget::Widget;

#[derive(Clone, Debug, PartialEq)]
enum RenderMode {
    Static,
    Dynamic,
    Raw,
}

pub struct PipeWidget {
    config: BTreeMap<String, PipeConfig>,
    zj_conf: BTreeMap<String, String>,
}

#[derive(Clone)]
struct PipeConfig {
    format: Vec<FormattedPart>,
    render_mode: RenderMode,
    truncate: bool,
    overflow: String,
    scrollable: bool,
    scroll_step: usize,
}

impl PipeWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        Self {
            config: parse_config(config),
            zj_conf: config.clone(),
        }
    }
}

impl Widget for PipeWidget {
    fn process(&self, name: &str, state: &crate::config::ZellijState) -> String {
        let pipe_config = match self.config.get(name) {
            Some(pc) => pc,
            None => {
                tracing::debug!("pipe no name {name}");
                return "".to_owned();
            }
        };

        let pipe_result = match state.pipe_results.get(name) {
            Some(pr) => pr,
            None => {
                tracing::debug!("pipe no content {name}");
                return "".to_owned();
            }
        };

        let content = pipe_config
            .format
            .iter()
            .map(|f| {
                let mut content = f.content.clone();

                if content.contains("{output}") {
                    content = content.replace(
                        "{output}",
                        pipe_result.strip_suffix('\n').unwrap_or(pipe_result),
                    )
                }

                (f, content)
            })
            .fold("".to_owned(), |acc, (f, content)| {
                if pipe_config.render_mode == RenderMode::Static {
                    return format!("{acc}{}", f.format_string(&content));
                }

                format!("{acc}{}", content)
            });

        match pipe_config.render_mode {
            RenderMode::Static => content,
            RenderMode::Dynamic => render_dynamic_formatted_content(&content, &self.zj_conf),
            RenderMode::Raw => pipe_result.to_owned(),
        }
    }

    fn process_click(&self, _name: &str, _state: &crate::config::ZellijState, _pos: usize) {}

    fn is_truncatable(&self, name: &str) -> bool {
        self.config
            .get(name)
            .map(|pipe_config| pipe_config.truncate)
            .unwrap_or(false)
    }

    fn process_scroll(
        &self,
        name: &str,
        state: &mut crate::config::ZellijState,
        delta: isize,
    ) -> bool {
        let Some(pipe_config) = self.config.get(name) else {
            return false;
        };
        if !pipe_config.scrollable {
            return false;
        }

        let current_offset = state.pipe_scroll_offsets.get(name).copied().unwrap_or(0);
        let max_offset = console::measure_text_width(&self.process(name, state)).saturating_sub(1);
        let step = pipe_config.scroll_step * delta.unsigned_abs().max(1);
        let next_offset = if delta.is_negative() {
            current_offset.saturating_sub(step)
        } else {
            current_offset.saturating_add(step)
        }
        .min(max_offset);

        state
            .pipe_scroll_offsets
            .insert(name.to_owned(), next_offset);
        true
    }

    fn truncate(
        &self,
        name: &str,
        output: &str,
        max_width: usize,
        state: &crate::config::ZellijState,
    ) -> String {
        let overflow = self
            .config
            .get(name)
            .map(|pipe_config| pipe_config.overflow.as_str())
            .unwrap_or("...");
        let offset = self
            .config
            .get(name)
            .filter(|pipe_config| pipe_config.scrollable)
            .and_then(|_| state.pipe_scroll_offsets.get(name).copied())
            .unwrap_or(0);
        truncate_ansi_string_to_width_from(output, overflow, max_width, offset)
    }
}

fn render_dynamic_formatted_content(content: &str, config: &BTreeMap<String, String>) -> String {
    formatted_parts_from_string_cached(content, config)
        .iter()
        .map(|fp| fp.format_string(&fp.content))
        .collect::<Vec<String>>()
        .join("")
}

fn parse_config(zj_conf: &BTreeMap<String, String>) -> BTreeMap<String, PipeConfig> {
    let mut keys: Vec<String> = zj_conf
        .keys()
        .filter(|k| k.starts_with("pipe_"))
        .cloned()
        .collect();
    keys.sort();

    let mut config: BTreeMap<String, PipeConfig> = BTreeMap::new();

    for key in keys {
        let Some((pipe_name, suffix)) = split_pipe_key(&key) else {
            continue;
        };
        let mut pipe_conf = PipeConfig {
            format: vec![],
            render_mode: RenderMode::Static,
            truncate: false,
            overflow: "...".to_owned(),
            scrollable: false,
            scroll_step: 4,
        };

        if let Some(existing_conf) = config.get(pipe_name) {
            pipe_conf = existing_conf.clone();
        }

        if suffix == "format" {
            pipe_conf.format =
                FormattedPart::multiple_from_format_string(zj_conf.get(&key).unwrap(), zj_conf);
        }

        if suffix == "rendermode" {
            pipe_conf.render_mode = match zj_conf.get(&key) {
                Some(mode) => match mode.as_str() {
                    "static" => RenderMode::Static,
                    "dynamic" => RenderMode::Dynamic,
                    "raw" => RenderMode::Raw,
                    _ => RenderMode::Static,
                },
                None => RenderMode::Static,
            };
        }

        if suffix == "truncate" {
            pipe_conf.truncate = zj_conf.get(&key).map(|v| v == "true").unwrap_or(false);
        }

        if suffix == "overflow"
            && let Some(overflow) = zj_conf.get(&key)
        {
            pipe_conf.overflow.clone_from(overflow);
        }

        if suffix == "scrollable" {
            pipe_conf.scrollable = zj_conf.get(&key).map(|v| v == "true").unwrap_or(false);
        }

        if suffix == "scroll_step" {
            pipe_conf.scroll_step = zj_conf
                .get(&key)
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(4)
                .max(1);
        }

        config.insert(pipe_name.to_owned(), pipe_conf);
    }
    config
}

fn split_pipe_key(key: &str) -> Option<(&str, &str)> {
    for suffix in [
        "scroll_step",
        "rendermode",
        "scrollable",
        "truncate",
        "overflow",
        "format",
    ] {
        let key_suffix = format!("_{suffix}");
        if let Some(pipe_name) = key.strip_suffix(&key_suffix) {
            return Some((pipe_name, suffix));
        }
    }
    None
}
