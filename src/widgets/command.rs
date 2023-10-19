use std::{
    collections::BTreeMap,
    fs::{self, remove_file, File},
    ops::Sub,
    path::Path,
    time::UNIX_EPOCH,
};

use chrono::{DateTime, Duration, Local};
use regex::Regex;
use zellij_tile::shim::run_command;

use crate::render::FormattedPart;

use super::widget::Widget;

const TIMESTAMP_FORMAT: &str = "%s";

#[derive(Clone, Debug)]
struct CommandConfig {
    command: String,
    format: FormattedPart,
    interval: i64,
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
    pub fn new(config: BTreeMap<String, String>) -> Self {
        Self {
            config: parse_config(config),
        }
    }
}

impl Widget for CommandWidget {
    fn process(&self, name: &str, state: crate::ZellijState) -> String {
        let command_config = match self.config.get(name) {
            Some(cc) => cc,
            None => {
                return "".to_string();
            }
        };

        run_command_if_needed(command_config.clone(), name, state.clone());

        let command_result = match state.command_results.get(name) {
            Some(cr) => cr,
            None => {
                return "".to_string();
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
                command_result.stdout.strip_suffix('\n').unwrap(),
            );
        }

        if content.contains("{stderr}") {
            content = content.replace(
                "{stderr}",
                command_result.stderr.strip_suffix('\n').unwrap(),
            );
        }

        command_config.format.format_string(content)
    }

    fn process_click(&self, _state: crate::ZellijState, _pos: usize) {}
}

fn run_command_if_needed(command_config: CommandConfig, name: &str, state: crate::ZellijState) {
    let ts = Local::now();
    let last_run =
        get_timestamp_from_event_or_default(name, state.clone(), command_config.interval);

    if ts.timestamp() - last_run.timestamp() >= command_config.interval {
        let mut context = BTreeMap::new();
        context.insert("name".to_owned(), name.to_owned());
        context.insert(
            "timestamp".to_owned(),
            ts.format(TIMESTAMP_FORMAT).to_string(),
        );

        let command = command_config.command.split(' ').collect::<Vec<&str>>();
        run_command(&command, context);
    }
}

fn parse_config(zj_conf: BTreeMap<String, String>) -> BTreeMap<String, CommandConfig> {
    let mut keys: Vec<String> = zj_conf
        .keys()
        .cloned()
        .filter(|k| k.starts_with("command_"))
        .collect();
    keys.sort();

    let mut config: BTreeMap<String, CommandConfig> = BTreeMap::new();

    let key_name_regex = Regex::new("_[a-zA-Z0-9]+$").unwrap();
    for key in keys {
        let command_name = key_name_regex.replace(&key, "").to_string();

        let mut command_conf = CommandConfig {
            command: "".to_owned(),
            format: FormattedPart::default(),
            interval: 1,
        };

        if let Some(existing_conf) = config.get(command_name.as_str()) {
            command_conf = existing_conf.clone();
        }

        if key.ends_with("command") {
            command_conf.command = zj_conf.get(&key).unwrap().to_string().clone();
        }

        if key.ends_with("format") {
            command_conf.format =
                FormattedPart::from_format_string(zj_conf.get(&key).unwrap().to_string().clone());
        }

        if key.ends_with("interval") {
            command_conf.interval = zj_conf.get(&key).unwrap().parse::<i64>().unwrap_or(1);
        }

        config.insert(command_name, command_conf);
    }

    config
}

fn get_timestamp_from_event_or_default(
    name: &str,
    state: crate::ZellijState,
    interval: i64,
) -> DateTime<Local> {
    let command_result = state.command_results.get(name);
    if command_result.is_none() {
        if lock(name, state.clone(), interval) {
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

    if Local::now().timestamp() - state.start_time.timestamp() < 10 {
        release(name, state.clone());
    }

    match DateTime::parse_from_str(ts_context, TIMESTAMP_FORMAT) {
        Ok(ts) => ts.into(),
        Err(_) => Sub::<Duration>::sub(Local::now(), Duration::days(1)),
    }
}

fn lock(name: &str, state: crate::ZellijState, interval: i64) -> bool {
    let path = format!("/tmp/{}.{}.lock", state.plugin_uuid, name);

    if !Path::new(&path).exists() {
        let _ = File::create(path);

        return false;
    }

    // refresh lock, when it's older than the interval for another try.
    // This must be done when reattaching the session since the command
    // otherwise won't run because the permissions are received too late
    if let Ok(metadata) = fs::metadata(path.clone()) {
        if let Ok(time) = metadata.modified() {
            let seconds = time.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

            if Local::now().timestamp() - seconds > interval {
                release(name, state);
                let _ = File::create(path);

                return false;
            }
        }
    }

    true
}

fn release(name: &str, state: crate::ZellijState) {
    let path = format!("/tmp/{}.{}.lock", state.plugin_uuid, name);

    if Path::new(&path).exists() {
        let _ = remove_file(path);
    }
}
