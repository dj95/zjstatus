use lazy_static::lazy_static;
use regex::Regex;
use std::collections::BTreeMap;

use crate::render::FormattedPart;

use super::widget::Widget;

lazy_static! {
    static ref PIPE_REGEX: Regex = Regex::new("_[a-zA-Z0-9]+$").unwrap();
}

pub struct PipeWidget {
    config: BTreeMap<String, PipeConfig>,
}

#[derive(Clone)]
struct PipeConfig {
    format: Vec<FormattedPart>,
}

impl PipeWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        Self {
            config: parse_config(config),
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

        pipe_config
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
                format!("{acc}{}", f.format_string(&content))
            })
    }

    fn process_click(&self, _name: &str, _state: &crate::config::ZellijState, _pos: usize) {}
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
        let mut pipe_conf = PipeConfig { format: vec![] };

        if let Some(existing_conf) = config.get(pipe_name.as_str()) {
            pipe_conf = existing_conf.clone();
        }

        if key.ends_with("format") {
            pipe_conf.format =
                FormattedPart::multiple_from_format_string(zj_conf.get(&key).unwrap(), zj_conf);
        }

        config.insert(pipe_name, pipe_conf);
    }
    config
}
