use lazy_static::lazy_static;
use std::{
    collections::BTreeMap,
    fs::{remove_file, File},
    ops::Sub,
    path::Path,
};

use chrono::{DateTime, Duration, Local};
use regex::Regex;
#[cfg(not(feature = "bench"))]
use zellij_tile::shim::run_command;

use crate::render::{formatted_parts_from_string_cached, FormattedPart};

use crate::{config::ZellijState, widgets::widget::Widget};

pub const TIMESTAMP_FORMAT: &str = "%s";

lazy_static! {
    static ref COMMAND_REGEX: Regex = Regex::new("_[a-zA-Z0-9]+$").unwrap();
}

#[derive(Clone, Debug)]
enum RenderMode {
    Static,
    Dynamic,
    Raw,
}

#[derive(Clone, Debug)]
struct CommandConfig {
    command: String,
    format: FormattedPart,
    interval: i64,
    render_mode: RenderMode,
}

#[derive(Clone, Debug)]
pub struct CommandResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub context: BTreeMap<String, String>,
}

pub struct CommandWidget {
    config: BTreeMap<String, CommandConfig>,
}

impl CommandWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        Self {
            config: parse_config(config),
        }
    }
}

impl Widget for CommandWidget {
    fn process(&self, name: &str, state: &ZellijState) -> String {
        let command_config = match self.config.get(name) {
            Some(cc) => cc,
            None => {
                return "".to_owned();
            }
        };

        run_command_if_needed(command_config.clone(), name, state);

        let command_result = match state.command_results.get(name) {
            Some(cr) => cr,
            None => {
                return "".to_owned();
            }
        };

        let mut content = command_config.format.content.clone();

        if content.contains("{exit_code}") {
            content = content.replace(
                "{exit_code}",
                format!("{}", command_result.exit_code.unwrap_or(-1)).as_str(),
            );
        }

        if content.contains("{stdout}") {
            content = content.replace(
                "{stdout}",
                command_result
                    .stdout
                    .strip_suffix('\n')
                    .unwrap_or(&command_result.stdout),
            );
        }

        if content.contains("{stderr}") {
            content = content.replace(
                "{stderr}",
                command_result
                    .stderr
                    .strip_suffix('\n')
                    .unwrap_or(&command_result.stderr),
            );
        }

        match command_config.render_mode {
            RenderMode::Static => command_config.format.format_string(&content),
            RenderMode::Dynamic => render_dynamic_formatted_content(&content),
            RenderMode::Raw => content,
        }
    }

    fn process_click(&self, _state: &ZellijState, _pos: usize) {}
}

fn render_dynamic_formatted_content(content: &str) -> String {
    formatted_parts_from_string_cached(content)
        .iter()
        .map(|fp| fp.format_string(&fp.content))
        .collect::<Vec<String>>()
        .join("")
}

fn run_command_if_needed(command_config: CommandConfig, name: &str, state: &ZellijState) {
    let ts = Local::now();
    let last_run = get_timestamp_from_event_or_default(name, state, command_config.interval);

    if ts.timestamp() - last_run.timestamp() >= command_config.interval {
        let mut context = BTreeMap::new();
        context.insert("name".to_owned(), name.to_owned());
        context.insert(
            "timestamp".to_owned(),
            ts.format(TIMESTAMP_FORMAT).to_string(),
        );

        let command = commandline_parser(&command_config.command);
        #[cfg(not(feature = "bench"))]
        run_command(
            &command.iter().map(|x| x.as_str()).collect::<Vec<&str>>(),
            context,
        );
    }
}

fn parse_config(zj_conf: &BTreeMap<String, String>) -> BTreeMap<String, CommandConfig> {
    let mut keys: Vec<String> = zj_conf
        .keys()
        .filter(|k| k.starts_with("command_"))
        .cloned()
        .collect();
    keys.sort();

    let mut config: BTreeMap<String, CommandConfig> = BTreeMap::new();

    for key in keys {
        let command_name = COMMAND_REGEX.replace(&key, "").to_string();

        let mut command_conf = CommandConfig {
            command: "".to_owned(),
            format: FormattedPart::default(),
            interval: 1,
            render_mode: RenderMode::Static,
        };

        if let Some(existing_conf) = config.get(command_name.as_str()) {
            command_conf = existing_conf.clone();
        }

        if key.ends_with("command") {
            command_conf.command = zj_conf.get(&key).unwrap().to_owned().clone();
        }

        if key.ends_with("format") {
            command_conf.format = FormattedPart::from_format_string(zj_conf.get(&key).unwrap());
        }

        if key.ends_with("interval") {
            command_conf.interval = zj_conf.get(&key).unwrap().parse::<i64>().unwrap_or(1);
        }

        if key.ends_with("rendermode") {
            command_conf.render_mode = match zj_conf.get(&key) {
                Some(mode) => match mode.as_str() {
                    "static" => RenderMode::Static,
                    "dynamic" => RenderMode::Dynamic,
                    "raw" => RenderMode::Raw,
                    _ => RenderMode::Static,
                },
                None => RenderMode::Static,
            };
        }

        config.insert(command_name, command_conf);
    }

    config
}

fn get_timestamp_from_event_or_default(
    name: &str,
    state: &ZellijState,
    interval: i64,
) -> DateTime<Local> {
    let command_result = state.command_results.get(name);
    if command_result.is_none() {
        if lock(name, state.clone()) {
            return Local::now();
        }

        return Sub::<Duration>::sub(Local::now(), Duration::days(1));
    }
    let command_result = command_result.unwrap();

    let ts_context = command_result.context.get("timestamp");
    if ts_context.is_none() {
        return Sub::<Duration>::sub(Local::now(), Duration::days(1));
    }
    let ts_context = ts_context.unwrap();

    if Local::now().timestamp() - state.start_time.timestamp() < interval {
        release(name, state.clone());
    }

    match DateTime::parse_from_str(ts_context, TIMESTAMP_FORMAT) {
        Ok(ts) => ts.into(),
        Err(_) => Sub::<Duration>::sub(Local::now(), Duration::days(1)),
    }
}

fn lock(name: &str, state: ZellijState) -> bool {
    let path = format!("/tmp/{}.{}.lock", state.plugin_uuid, name);

    if !Path::new(&path).exists() {
        let _ = File::create(path);

        return false;
    }

    true
}

fn release(name: &str, state: ZellijState) {
    let path = format!("/tmp/{}.{}.lock", state.plugin_uuid, name);

    if Path::new(&path).exists() {
        let _ = remove_file(path);
    }
}

fn commandline_parser(input: &str) -> Vec<String> {
    let mut output: Vec<String> = Vec::new();

    let special_chars = ['"', '\''];

    let mut found_special_char = '\0';
    let mut buffer = "".to_owned();
    let mut is_escaped = false;
    let mut is_in_group = false;

    for character in input.chars() {
        if is_escaped {
            is_escaped = false;
            buffer = format!("{}\\{}", buffer.to_owned(), character);
            continue;
        }

        if character == '\\' {
            is_escaped = true;
            continue;
        }

        if found_special_char == character && is_in_group {
            is_in_group = false;
            found_special_char = '\0';
            output.push(buffer.clone());
            buffer = "".to_owned();
            continue;
        }

        if special_chars.contains(&character) && !is_in_group {
            is_in_group = true;
            found_special_char = character;
            continue;
        }

        if character == ' ' && !is_in_group {
            output.push(buffer.clone());
            buffer = "".to_owned();
            continue;
        }

        buffer = format!("{}{}", buffer, character);
    }

    if !buffer.is_empty() {
        output.push(buffer.clone());
    }

    output
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_commandline_parser() {
        let input = "pwd";
        let result = commandline_parser(input);
        let expected = Vec::from(["pwd"]);
        assert_eq!(result, expected);

        let input = "bash -c \"pwd | base64 -c \\\"bla\\\"\"";
        let result = commandline_parser(input);
        let expected = Vec::from(["bash", "-c", "pwd | base64 -c \\\"bla\\\""]);
        assert_eq!(result, expected);

        let input = "bash -c \"pwd | base64 -c 'bla' | xxd\"";
        let result = commandline_parser(input);
        let expected = Vec::from(["bash", "-c", "pwd | base64 -c 'bla' | xxd"]);
        assert_eq!(result, expected);
    }
}
