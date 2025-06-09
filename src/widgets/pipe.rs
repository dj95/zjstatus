use lazy_static::lazy_static;
use regex::Regex;
use std::collections::BTreeMap;

use crate::render::{FormattedPart, formatted_parts_from_string_cached};

use super::widget::Widget;

lazy_static! {
    static ref PIPE_REGEX: Regex = Regex::new("_[a-zA-Z0-9]+$").unwrap();
}

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
        let pipe_name = PIPE_REGEX.replace(&key, "").to_string();
        let mut pipe_conf = PipeConfig {
            format: vec![],
            render_mode: RenderMode::Static,
        };

        if let Some(existing_conf) = config.get(pipe_name.as_str()) {
            pipe_conf = existing_conf.clone();
        }

        if key.ends_with("format") {
            pipe_conf.format =
                FormattedPart::multiple_from_format_string(zj_conf.get(&key).unwrap(), zj_conf);
        }

        if key.ends_with("rendermode") {
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

        config.insert(pipe_name, pipe_conf);
    }
    config
}
