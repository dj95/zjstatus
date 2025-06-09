use kdl::{KdlDocument, KdlError};
use lazy_static::lazy_static;
use std::{
    collections::BTreeMap,
    fs::{remove_file, File},
    ops::Sub,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Duration, Local};
use regex::Regex;
#[cfg(all(not(feature = "bench"), not(test)))]
use zellij_tile::shim::{run_command, run_command_with_env_variables_and_cwd};

use crate::render::{formatted_parts_from_string_cached, FormattedPart};

use crate::{config::ZellijState, widgets::widget::Widget};

pub const TIMESTAMP_FORMAT: &str = "%s";

lazy_static! {
    static ref COMMAND_REGEX: Regex = Regex::new("_[a-zA-Z0-9]+$").unwrap();
}

#[derive(Clone, Debug, PartialEq)]
enum RenderMode {
    Static,
    Dynamic,
    Raw,
}

#[derive(Clone, Debug)]
struct CommandConfig {
    command: String,
    format: Vec<FormattedPart>,
    env: Option<BTreeMap<String, String>>,
    cwd: Option<PathBuf>,
    interval: i64,
    render_mode: RenderMode,
    click_action: String,
}

#[derive(Clone, Debug, Default)]
pub struct CommandResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub context: BTreeMap<String, String>,
}

pub struct CommandWidget {
    config: BTreeMap<String, CommandConfig>,
    zj_conf: BTreeMap<String, String>,
}

impl CommandWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        Self {
            config: parse_config(config),
            zj_conf: config.clone(),
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

        let content = command_config
            .format
            .iter()
            .map(|f| {
                let mut content = f.content.clone();

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

                (f, content)
            })
            .fold("".to_owned(), |acc, (f, content)| {
                if command_config.render_mode == RenderMode::Static {
                    return format!("{acc}{}", f.format_string(&content));
                }

                format!("{acc}{}", content)
            });

        match command_config.render_mode {
            RenderMode::Static => content,
            RenderMode::Dynamic => render_dynamic_formatted_content(&content, &self.zj_conf),
            RenderMode::Raw => content,
        }
    }

    fn process_click(&self, name: &str, _state: &ZellijState, _pos: usize) {
        let command_config = match self.config.get(name) {
            Some(cc) => cc,
            None => {
                return;
            }
        };

        if command_config.click_action.is_empty() {
            return;
        }

        let command = commandline_parser(&command_config.click_action);
        let context: BTreeMap<String, String> = BTreeMap::new();

        tracing::debug!("Running command {:?} {:?}", command, context);

        #[cfg(all(not(feature = "bench"), not(test)))]
        run_command(
            &command.iter().map(|x| x.as_str()).collect::<Vec<&str>>(),
            context,
        );
    }
}

fn render_dynamic_formatted_content(content: &str, config: &BTreeMap<String, String>) -> String {
    formatted_parts_from_string_cached(content, config)
        .iter()
        .map(|fp| fp.format_string(&fp.content))
        .collect::<Vec<String>>()
        .join("")
}

#[tracing::instrument(skip(command_config, state))]
fn run_command_if_needed(command_config: CommandConfig, name: &str, state: &ZellijState) -> bool {
    let got_result = state.command_results.contains_key(name);
    if got_result && command_config.interval == 0 {
        return false;
    }

    let ts = Local::now();
    let last_run = get_timestamp_from_event_or_default(name, state, command_config.interval);

    if ts.timestamp() - last_run.timestamp() >= command_config.interval {
        let mut context = BTreeMap::new();
        context.insert("name".to_owned(), name.to_owned());
        context.insert(
            "timestamp".to_owned(),
            ts.format(TIMESTAMP_FORMAT).to_string(),
        );

        #[allow(unused_variables)]
        let command = commandline_parser(&command_config.command);
        tracing::debug!("Running command: {:?}", command);

        if command_config.env.is_some() || command_config.cwd.is_some() {
            #[cfg(all(not(feature = "bench"), not(test)))]
            run_command_with_env_variables_and_cwd(
                &command.iter().map(|x| x.as_str()).collect::<Vec<&str>>(),
                command_config.env.unwrap(),
                command_config.cwd.unwrap(),
                context,
            );

            return true;
        }

        #[cfg(all(not(feature = "bench"), not(test)))]
        run_command(
            &command.iter().map(|x| x.as_str()).collect::<Vec<&str>>(),
            context,
        );

        return true;
    }

    false
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
            format: Vec::new(),
            cwd: None,
            env: None,
            interval: 1,
            render_mode: RenderMode::Static,
            click_action: "".to_owned(),
        };

        if let Some(existing_conf) = config.get(command_name.as_str()) {
            command_conf = existing_conf.clone();
        }

        if key.ends_with("command") {
            command_conf
                .command
                .clone_from(&zj_conf.get(&key).unwrap().to_owned());
        }

        if key.ends_with("clickaction") {
            command_conf
                .click_action
                .clone_from(&zj_conf.get(&key).unwrap().to_owned());
        }

        if key.ends_with("env") {
            let doc: Result<KdlDocument, KdlError> = zj_conf.get(&key).unwrap().parse();

            if let Ok(doc) = doc {
                command_conf.env = Some(get_env_vars(doc));
            }
        }

        if key.ends_with("cwd") {
            let mut cwd = PathBuf::new();
            cwd.push(zj_conf.get(&key).unwrap().to_owned().clone());

            command_conf.cwd = Some(cwd);
        }

        if key.ends_with("format") {
            command_conf.format =
                FormattedPart::multiple_from_format_string(zj_conf.get(&key).unwrap(), zj_conf);
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

fn get_env_vars(doc: KdlDocument) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();

    for n in doc.nodes() {
        let children = n.entries();
        if children.len() != 1 {
            continue;
        }

        let value = match children.first().unwrap().value().as_string() {
            Some(value) => value,
            None => continue,
        };

        output.insert(n.name().value().to_string(), value.to_string());
    }

    output
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

        return Sub::<Duration>::sub(Local::now(), Duration::try_days(1).unwrap());
    }
    let command_result = command_result.unwrap();

    let ts_context = command_result.context.get("timestamp");
    if ts_context.is_none() {
        return Sub::<Duration>::sub(Local::now(), Duration::try_days(1).unwrap());
    }
    let ts_context = ts_context.unwrap();

    if Local::now().timestamp() - state.start_time.timestamp() < interval {
        release(name, state.clone());
    }

    match DateTime::parse_from_str(ts_context, TIMESTAMP_FORMAT) {
        Ok(ts) => ts.into(),
        Err(_) => Sub::<Duration>::sub(Local::now(), Duration::try_days(1).unwrap()),
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
            "".clone_into(&mut buffer);
            continue;
        }

        if special_chars.contains(&character) && !is_in_group {
            is_in_group = true;
            found_special_char = character;
            continue;
        }

        if character == ' ' && !is_in_group {
            output.push(buffer.clone());
            "".clone_into(&mut buffer);
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
    use rstest::rstest;

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

    #[rstest]
    // no result, interval 1 second
    #[case(1, &ZellijState::default(), true)]
    // only run once without a result
    #[case(0, &ZellijState::default(), true)]
    // do not run with run once and result
    #[case(0, &ZellijState {
        command_results: BTreeMap::from([(
            "test".to_owned(),
            CommandResult::default(),
        )]),
        ..ZellijState::default()
    }, false)]
    // run if interval is exceeded
    #[case(1, &ZellijState {
        command_results: BTreeMap::from([(
            "test".to_owned(),
            CommandResult{
                context: BTreeMap::from([("timestamp".to_owned(), "0".to_owned())]),
                ..CommandResult::default()
            }
        )]),
        ..ZellijState::default()
    }, true)]
    // do not run if interval is not exceeded
    #[case(1, &ZellijState {
        command_results: BTreeMap::from([(
            "test".to_owned(),
            CommandResult{
                context: BTreeMap::from([("timestamp".to_owned(), Local::now().format(TIMESTAMP_FORMAT).to_string())]),
                ..CommandResult::default()
            }
        )]),
        ..ZellijState::default()
    }, false)]
    pub fn test_run_command_if_needed(
        #[case] interval: i64,
        #[case] state: &ZellijState,
        #[case] expected: bool,
    ) {
        let res = run_command_if_needed(
            CommandConfig {
                command: "echo test".to_owned(),
                format: Vec::new(),
                env: None,
                cwd: None,
                interval,
                render_mode: RenderMode::Static,
                click_action: "".to_owned(),
            },
            "test",
            state,
        );
        assert_eq!(res, expected);
    }
}
