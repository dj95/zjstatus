//! Tab groups: tabs named `prefix: name` are grouped under `prefix`.
//!
//! Every tab runs its own instance of the bar. The collapsed groups are kept
//! in a file in the plugin's `/cache` folder, which all instances share: the
//! instance that changes them writes the file, and the others read it again
//! on tab updates and on their timer.

use std::collections::{BTreeMap, HashMap};

use zellij_tile::prelude::{InputMode, PaneManifest, TabInfo};

use crate::config::ZellijState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CollapseMode {
    /// Groups are expanded and collapsed by hand.
    Manual,
    /// Only the group of the active tab is expanded.
    Auto,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupsConfig {
    pub delimiter: String,
    pub min_size: usize,
    pub mode: CollapseMode,
    pub start_collapsed: bool,
    pub keep_together: bool,
}

impl GroupsConfig {
    /// `None` unless `tab_groups` is "true".
    pub fn new(config: &BTreeMap<String, String>) -> Option<Self> {
        if config.get("tab_groups").map(String::as_str) != Some("true") {
            return None;
        }
        let delimiter = config
            .get("tab_group_delimiter")
            .filter(|delimiter| !delimiter.is_empty())
            .cloned()
            .unwrap_or_else(|| ":".to_owned());
        let mode = match config.get("tab_group_collapse").map(String::as_str) {
            Some("auto") => CollapseMode::Auto,
            _ => CollapseMode::Manual,
        };
        Some(Self {
            delimiter,
            min_size: 2,
            mode,
            start_collapsed: config.get("tab_group_start_collapsed").map(String::as_str)
                == Some("true"),
            keep_together: config.get("tab_group_keep_together").map(String::as_str)
                != Some("false"),
        })
    }
}

/// Splits `prefix: name` into its trimmed prefix and name, if both are
/// non-empty.
pub fn split_prefix<'a>(name: &'a str, delimiter: &str) -> Option<(&'a str, &'a str)> {
    let (prefix, rest) = name.split_once(delimiter)?;
    let (prefix, rest) = (prefix.trim(), rest.trim());
    (!prefix.is_empty() && !rest.is_empty()).then_some((prefix, rest))
}

#[derive(Debug, PartialEq)]
pub struct Group<'a> {
    pub name: String,
    /// The tabs, with their names without the prefix.
    pub members: Vec<(&'a TabInfo, String)>,
}

impl<'a> Group<'a> {
    pub fn active(&self) -> Option<&'a TabInfo> {
        self.members
            .iter()
            .map(|(tab, _)| *tab)
            .find(|tab| tab.active)
    }
}

/// What the tab bar shows at the top level: plain tabs and groups.
#[derive(Debug, PartialEq)]
pub enum Item<'a> {
    Tab(&'a TabInfo),
    Group(Group<'a>),
}

impl<'a> Item<'a> {
    pub fn tabs(&self) -> Vec<&'a TabInfo> {
        match self {
            Item::Tab(tab) => vec![*tab],
            Item::Group(group) => group.members.iter().map(|(tab, _)| *tab).collect(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.tabs().iter().any(|tab| tab.active)
    }
}

/// Groups the tabs. A prefix shared by fewer than `min_size` tabs makes no
/// group. With `keep_together`, a group gathers all its tabs at the place of
/// its first one (the tabs are then moved there, see `next_move`); otherwise
/// only tabs next to each other are grouped.
pub fn build_items<'a>(tabs: &'a [TabInfo], config: &GroupsConfig) -> Vec<Item<'a>> {
    let mut sorted: Vec<&TabInfo> = tabs.iter().collect();
    sorted.sort_by_key(|tab| tab.position);
    let prefixed: Vec<(&TabInfo, Option<(&str, &str)>)> = sorted
        .into_iter()
        .map(|tab| (tab, split_prefix(&tab.name, &config.delimiter)))
        .collect();

    if config.keep_together {
        let mut sizes: HashMap<&str, usize> = HashMap::new();
        for (_, split) in &prefixed {
            if let Some((prefix, _)) = split {
                *sizes.entry(prefix).or_default() += 1;
            }
        }
        let mut items: Vec<Item> = vec![];
        let mut group_index: HashMap<&str, usize> = HashMap::new();
        for (tab, split) in prefixed {
            match split {
                Some((prefix, rest)) if sizes[prefix] >= config.min_size => {
                    let member = (tab, rest.to_owned());
                    match group_index.get(prefix) {
                        Some(index) => {
                            if let Item::Group(group) = &mut items[*index] {
                                group.members.push(member);
                            }
                        }
                        None => {
                            group_index.insert(prefix, items.len());
                            items.push(Item::Group(Group {
                                name: prefix.to_owned(),
                                members: vec![member],
                            }));
                        }
                    }
                }
                _ => items.push(Item::Tab(tab)),
            }
        }
        return items;
    }

    let mut items = vec![];
    let mut run: Vec<(&TabInfo, &str, &str)> = vec![];
    for (tab, split) in prefixed {
        match split {
            Some((prefix, rest)) => {
                if run.first().is_some_and(|(_, first, _)| *first != prefix) {
                    flush_run(&mut run, &mut items, config.min_size);
                }
                run.push((tab, prefix, rest));
            }
            None => {
                flush_run(&mut run, &mut items, config.min_size);
                items.push(Item::Tab(tab));
            }
        }
    }
    flush_run(&mut run, &mut items, config.min_size);
    items
}

/// Adds a run of neighbouring tabs with the same prefix: a group if it is
/// big enough, plain tabs otherwise.
fn flush_run<'a>(
    run: &mut Vec<(&'a TabInfo, &'a str, &'a str)>,
    items: &mut Vec<Item<'a>>,
    min_size: usize,
) {
    if run.len() >= min_size {
        items.push(Item::Group(Group {
            name: run[0].1.to_owned(),
            members: run
                .iter()
                .map(|(tab, _, rest)| (*tab, rest.to_string()))
                .collect(),
        }));
    } else {
        items.extend(run.iter().map(|(tab, _, _)| Item::Tab(tab)));
    }
    run.clear();
}

/// The groups commands act on: the shown groups, and the one holding the
/// active tab.
#[derive(Debug, Default, PartialEq)]
pub struct Shown {
    pub groups: Vec<String>,
    pub active: Option<String>,
}

pub fn shown_groups(tabs: &[TabInfo], config: &GroupsConfig) -> Shown {
    let mut shown = Shown::default();
    for item in build_items(tabs, config) {
        if let Item::Group(group) = item {
            if group.active().is_some() {
                shown.active = Some(group.name.clone());
            }
            shown.groups.push(group.name);
        }
    }
    shown
}

/// A move that brings the tabs of each group closer together.
#[derive(Clone, Debug, PartialEq)]
pub struct Move {
    pub tab_id: usize,
    pub position: usize,
}

/// The next tab to move, and where, so that the tabs of each group end up next
/// to each other; `None` when they already are. It is the first tab out of
/// place in the target order, which moves left to its place. Every instance of
/// the bar computes the same move, and the move keeps the target order, so
/// repeating this converges in at most one move per tab.
pub fn next_move(tabs: &[TabInfo], config: &GroupsConfig) -> Option<Move> {
    let gathering = GroupsConfig {
        keep_together: true,
        ..config.clone()
    };
    let target: Vec<&TabInfo> = build_items(tabs, &gathering)
        .iter()
        .flat_map(|item| item.tabs())
        .collect();
    let mut current: Vec<&TabInfo> = tabs.iter().collect();
    current.sort_by_key(|tab| tab.position);
    current
        .iter()
        .zip(target.iter())
        .position(|(now, wanted)| now.tab_id != wanted.tab_id)
        .map(|position| Move {
            tab_id: target[position].tab_id,
            position,
        })
}

/// Which groups are collapsed: a default and per-group choices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollapseState {
    /// Set by collapse_all and expand_all; overrides `tab_group_start_collapsed`.
    pub default: Option<bool>,
    pub groups: BTreeMap<String, bool>,
}

impl CollapseState {
    pub fn is_collapsed(&self, group: &str, start_collapsed: bool) -> bool {
        self.groups
            .get(group)
            .copied()
            .unwrap_or(self.default.unwrap_or(start_collapsed))
    }

    pub fn set(&mut self, group: &str, collapsed: bool) {
        self.groups.insert(group.to_owned(), collapsed);
    }

    pub fn set_all(&mut self, collapsed: bool) {
        self.default = Some(collapsed);
        self.groups.clear();
    }

    /// One line per entry: `default collapsed`, `collapsed NAME`, ...
    pub fn serialize(&self) -> String {
        let word = |collapsed: bool| if collapsed { "collapsed" } else { "expanded" };
        let mut lines = vec![];
        if let Some(default) = self.default {
            lines.push(format!("default {}", word(default)));
        }
        for (group, collapsed) in &self.groups {
            lines.push(format!("{} {}", word(*collapsed), group));
        }
        lines.join("\n")
    }

    pub fn parse(text: &str) -> Self {
        let mut state = Self::default();
        for line in text.lines() {
            let Some((word, rest)) = line.split_once(' ') else {
                continue;
            };
            let collapsed = match word {
                "collapsed" => true,
                "expanded" => false,
                "default" => {
                    state.default = match rest {
                        "collapsed" => Some(true),
                        "expanded" => Some(false),
                        _ => state.default,
                    };
                    continue;
                }
                _ => continue,
            };
            state.groups.insert(rest.to_owned(), collapsed);
        }
        state
    }
}

/// The part of the bar's state that the tab widget needs.
#[derive(Clone, Debug, Default)]
pub struct GroupsState {
    pub collapse: CollapseState,
    /// Whether the mouse event being handled is a left click (press). Clicks
    /// also send a release, which must not toggle a group back.
    pub left_click: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Toggle(Option<String>),
    Collapse(Option<String>),
    Expand(Option<String>),
    CollapseAll,
    ExpandAll,
    /// Collapses all groups if any is expanded, else expands them all.
    ToggleAll,
}

/// Parses `zjstatus::groups::<command>[::<group>]`. Without a group, a
/// command applies to the group of the active tab.
pub fn parse_command(line: &str) -> Option<Command> {
    let parts: Vec<&str> = line.trim().split("::").collect();
    if parts.len() < 3 || parts[0] != "zjstatus" || parts[1] != "groups" {
        return None;
    }
    let group = Some(parts[3..].join("::")).filter(|group| !group.is_empty());
    Some(match parts[2] {
        "toggle" => Command::Toggle(group),
        "collapse" => Command::Collapse(group),
        "expand" => Command::Expand(group),
        "collapse_all" => Command::CollapseAll,
        "expand_all" => Command::ExpandAll,
        "toggle_all" => Command::ToggleAll,
        _ => return None,
    })
}

/// Applies `command` to `state`. Returns whether anything changed.
pub fn apply(
    state: &mut CollapseState,
    command: &Command,
    shown: &Shown,
    config: &GroupsConfig,
) -> bool {
    let before = state.clone();
    let target = |group: &Option<String>| group.clone().or_else(|| shown.active.clone());
    match command {
        Command::Toggle(group) => {
            if let Some(group) = target(group) {
                let collapsed = state.is_collapsed(&group, config.start_collapsed);
                state.set(&group, !collapsed);
            }
        }
        Command::Collapse(group) => {
            if let Some(group) = target(group) {
                state.set(&group, true);
            }
        }
        Command::Expand(group) => {
            if let Some(group) = target(group) {
                state.set(&group, false);
            }
        }
        Command::CollapseAll => state.set_all(true),
        Command::ExpandAll => state.set_all(false),
        Command::ToggleAll => {
            if !shown.groups.is_empty() {
                let any_expanded = shown
                    .groups
                    .iter()
                    .any(|group| !state.is_collapsed(group, config.start_collapsed));
                state.set_all(any_expanded);
            }
        }
    }
    *state != before
}

/// The position of the tab that holds the plugin pane `plugin_id`.
pub fn own_tab_position(panes: &PaneManifest, plugin_id: u32) -> Option<usize> {
    panes.panes.iter().find_map(|(position, panes)| {
        panes
            .iter()
            .any(|pane| pane.is_plugin && pane.id == plugin_id)
            .then_some(*position)
    })
}

/// Reading and writing the collapse state file.
pub mod store {
    use super::CollapseState;

    /// The file for a session, in the plugin's `/cache` folder. Zellij 0.46
    /// gives each instance its own `/data` folder (its docs still say it is
    /// shared), but one `/cache` folder to all instances of a plugin URL, in
    /// every session, and keeps it when they unload.
    pub fn path(session: Option<&str>) -> Option<String> {
        let session: String = session?
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        Some(format!("/cache/tab-groups-{session}"))
    }

    pub fn read(path: &str) -> CollapseState {
        std::fs::read_to_string(path)
            .map(|text| CollapseState::parse(&text))
            .unwrap_or_default()
    }

    /// Writes the file atomically: other instances read it at any time, and
    /// must never see it half written.
    pub fn write(path: &str, state: &CollapseState) {
        let temporary = format!("{path}.{}.tmp", uuid::Uuid::new_v4());
        let written = std::fs::write(&temporary, state.serialize())
            .and_then(|_| std::fs::rename(&temporary, path));
        if let Err(error) = written {
            tracing::debug!("could not write {path}: {error}");
            let _ = std::fs::remove_file(&temporary);
        }
    }

    /// Sets one group in the file, keeping what other instances wrote.
    pub fn set(session: Option<&str>, group: &str, collapsed: bool) {
        if let Some(path) = path(session) {
            let mut state = read(&path);
            state.set(group, collapsed);
            write(&path, &state);
        }
    }
}

/// Moves a tab with the plugin API (`RunActionsAsUser`). Moving a tab to a
/// position is idempotent, so a repeated move does no harm.
fn move_tab(tab_move: &Move) {
    let action = zellij_tile::prelude::actions::Action::MoveTabToPosition {
        id: tab_move.tab_id as u64,
        position: tab_move.position as u64,
    };
    #[cfg(all(not(feature = "bench"), not(test)))]
    zellij_tile::prelude::run_action(action, BTreeMap::new());
    #[cfg(any(feature = "bench", test))]
    let _ = action;
}

/// The side of tab groups that runs in the plugin: shared state, keyboard
/// commands and keeping the tabs of a group together.
#[derive(Default)]
pub struct Controller {
    config: Option<GroupsConfig>,
    plugin_id: Option<u32>,
    /// The tab order in which this instance last moved a tab, and the move.
    /// The same move isn't repeated in the same order: if it worked, the order
    /// changes; if it didn't, repeating it wouldn't help.
    last_move: Option<(Vec<usize>, Move)>,
}

impl Controller {
    /// `plugin_id` is this instance's, to find its own tab.
    pub fn new(config: &BTreeMap<String, String>, plugin_id: u32) -> Self {
        Self {
            config: GroupsConfig::new(config),
            plugin_id: Some(plugin_id),
            ..Self::default()
        }
    }

    fn own_tab<'a>(&self, state: &'a ZellijState) -> Option<&'a TabInfo> {
        let position = own_tab_position(&state.panes, self.plugin_id?)?;
        state.tabs.iter().find(|tab| tab.position == position)
    }

    fn in_active_tab(&self, state: &ZellijState) -> bool {
        self.own_tab(state).is_some_and(|tab| tab.active)
    }

    /// Reads the shared state again. Returns whether it changed.
    pub fn reload(&self, state: &mut ZellijState) -> bool {
        if self.config.is_none() {
            return false;
        }
        let Some(path) = store::path(state.mode.session_name.as_deref()) else {
            return false;
        };
        let collapse = store::read(&path);
        if collapse == state.tab_groups.collapse {
            return false;
        }
        state.tab_groups.collapse = collapse;
        true
    }

    /// Handles `zjstatus::groups::...` lines. Every instance gets them; only
    /// the one in the active tab acts. Returns whether to render again.
    pub fn handle_pipe(&mut self, state: &mut ZellijState, input: &str) -> bool {
        let Some(config) = self.config.clone() else {
            return false;
        };
        let commands: Vec<Command> = input.lines().filter_map(parse_command).collect();
        if commands.is_empty() || config.mode == CollapseMode::Auto || !self.in_active_tab(state) {
            return false;
        }
        let Some(path) = store::path(state.mode.session_name.as_deref()) else {
            return false;
        };
        let shown = shown_groups(&state.tabs, &config);
        let mut collapse = store::read(&path);
        let mut changed = false;
        for command in &commands {
            changed |= apply(&mut collapse, command, &shown, &config);
        }
        if changed {
            store::write(&path, &collapse);
        }
        changed |= collapse != state.tab_groups.collapse;
        state.tab_groups.collapse = collapse;
        changed
    }

    /// After a tab update: if this instance is in the active tab, makes the
    /// next move that keeps the groups together. Returns the move.
    ///
    /// Only the active tab's bar moves tabs: Zellij sends no tab updates to the
    /// bar of a tab opened in the background until the tab is shown, so that
    /// bar can't move its own tab. With several clients, each client's active
    /// bar makes the same move, which puts a tab at a position, so they agree.
    pub fn keep_together(&mut self, state: &ZellijState) -> Option<Move> {
        let config = self.config.clone()?;
        if !config.keep_together
            || state.mode.mode == InputMode::RenameTab
            || !self.in_active_tab(state)
        {
            return None;
        }
        let tab_move = next_move(&state.tabs, &config)?;
        let mut order: Vec<&TabInfo> = state.tabs.iter().collect();
        order.sort_by_key(|tab| tab.position);
        let order: Vec<usize> = order.iter().map(|tab| tab.tab_id).collect();
        let attempt = (order, tab_move.clone());
        if self.last_move.as_ref() == Some(&attempt) {
            return None;
        }
        tracing::debug!("moving tab {tab_move:?} to keep its group together");
        move_tab(&tab_move);
        self.last_move = Some(attempt);
        Some(tab_move)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn config() -> GroupsConfig {
        GroupsConfig::new(&BTreeMap::from([(
            "tab_groups".to_owned(),
            "true".to_owned(),
        )]))
        .unwrap()
    }

    fn tabs(names: &[&str]) -> Vec<TabInfo> {
        names
            .iter()
            .enumerate()
            .map(|(position, name)| TabInfo {
                position,
                tab_id: position + 10,
                name: name.to_string(),
                active: name.ends_with('*'),
                ..TabInfo::default()
            })
            .collect()
    }

    /// Items as text: plain tabs by name, groups as `prefix[a, b]`.
    fn shape(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|item| match item {
                Item::Tab(tab) => tab.name.clone(),
                Item::Group(group) => format!(
                    "{}[{}]",
                    group.name,
                    group
                        .members
                        .iter()
                        .map(|(_, name)| name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            })
            .collect()
    }

    #[test]
    fn groups_are_off_unless_enabled() {
        assert_eq!(GroupsConfig::new(&BTreeMap::new()), None);
        let config = config();
        assert_eq!(config.delimiter, ":");
        assert_eq!(config.mode, CollapseMode::Manual);
        assert!(!config.start_collapsed && config.keep_together);
    }

    #[test]
    fn splits_prefix_and_name() {
        assert_eq!(split_prefix("api: auth", ":"), Some(("api", "auth")));
        assert_eq!(
            split_prefix(" api :auth: x ", ":"),
            Some(("api", "auth: x"))
        );
        assert_eq!(split_prefix("api/auth", "/"), Some(("api", "auth")));
        assert_eq!(split_prefix("api: auth", ": "), Some(("api", "auth")));
        assert_eq!(split_prefix("api:auth", ": "), None);
        assert_eq!(split_prefix("notes", ":"), None);
        assert_eq!(split_prefix("api:", ":"), None);
        assert_eq!(split_prefix(": auth", ":"), None);
    }

    #[test]
    fn a_prefix_needs_two_tabs_to_make_a_group() {
        let tabs = tabs(&["notes", "api: auth", "web: ui", "api: billing"]);

        assert_eq!(
            shape(&build_items(&tabs, &config())),
            ["notes", "api[auth, billing]", "web: ui"],
        );
    }

    #[test]
    fn without_keep_together_only_neighbours_are_grouped() {
        let config = GroupsConfig {
            keep_together: false,
            ..config()
        };
        let tabs = tabs(&["api: a", "api: b", "notes", "api: c", "web: x", "web: y"]);

        assert_eq!(
            shape(&build_items(&tabs, &config)),
            ["api[a, b]", "notes", "api: c", "web[x, y]"],
        );
    }

    #[test]
    fn items_follow_tab_positions() {
        let mut tabs = tabs(&["api: a", "notes", "api: b"]);
        tabs.reverse();

        assert_eq!(
            shape(&build_items(&tabs, &config())),
            ["api[a, b]", "notes"]
        );
    }

    #[test]
    fn finds_the_shown_groups_and_the_active_one() {
        let config = config();
        assert_eq!(
            shown_groups(
                &tabs(&["api: a", "solo: x", "api: b*", "web: c", "web: d"]),
                &config
            ),
            Shown {
                groups: names(&["api", "web"]),
                active: Some("api".to_owned()),
            }
        );
        assert_eq!(
            shown_groups(&tabs(&["api: a*", "web: b"]), &config),
            Shown::default()
        );
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    /// Applies `next_move` until nothing moves, like the bar instances do.
    fn settle(names: &[&str]) -> (Vec<String>, usize) {
        let config = config();
        let mut tabs = tabs(names);
        let mut moves = 0;
        while let Some(tab_move) = next_move(&tabs, &config) {
            let at = tabs
                .iter()
                .position(|tab| tab.tab_id == tab_move.tab_id)
                .unwrap();
            assert!(tab_move.position < at, "only moves tabs left");
            let tab = tabs.remove(at);
            tabs.insert(tab_move.position, tab);
            for (position, tab) in tabs.iter_mut().enumerate() {
                tab.position = position;
            }
            moves += 1;
            assert!(moves < 50, "does not converge");
        }
        (tabs.iter().map(|tab| tab.name.clone()).collect(), moves)
    }

    #[test]
    fn moves_group_tabs_next_to_their_group() {
        assert_eq!(
            settle(&["api: a", "notes", "api: b"]),
            (vec!["api: a".into(), "api: b".into(), "notes".into()], 1)
        );
        assert_eq!(
            settle(&["api: a", "api: b", "web: x", "notes", "api: c"]),
            (names(&["api: a", "api: b", "api: c", "web: x", "notes"]), 1),
            "a new tab at the end moves straight to its group",
        );
        assert_eq!(
            settle(&["api: a", "web: x", "web: y", "api: b"]).0,
            ["api: a", "api: b", "web: x", "web: y"],
        );
        assert_eq!(
            settle(&["api: a", "web: x", "api: b", "web: y", "api: c", "solo: z"]),
            (
                names(&["api: a", "api: b", "api: c", "web: x", "web: y", "solo: z"]),
                2
            ),
            "placing the api tabs also brings the web tabs together",
        );
    }

    #[test]
    fn nothing_moves_when_groups_are_together() {
        let config = config();
        assert_eq!(
            next_move(&tabs(&["notes", "api: a", "api: b", "web: x"]), &config),
            None
        );
        assert_eq!(
            next_move(&tabs(&["api: a", "notes", "web: x"]), &config),
            None
        );
    }

    #[test]
    fn collapse_state_survives_the_file() {
        let mut state = CollapseState::default();
        state.set("api", true);
        state.set("web stuff", false);
        state.default = Some(true);

        assert_eq!(CollapseState::parse(&state.serialize()), state);
        assert_eq!(
            CollapseState::parse("garbage\ncollapsed x"),
            CollapseState {
                default: None,
                groups: BTreeMap::from([("x".to_owned(), true)]),
            }
        );
    }

    /// Regression: the file was in `/data`, which Zellij gives each instance
    /// for itself, so each tab's bar kept its own collapsed groups.
    #[test]
    fn the_file_is_in_the_folder_all_instances_share() {
        assert_eq!(
            store::path(Some("main")).as_deref(),
            Some("/cache/tab-groups-main")
        );
        assert_eq!(
            store::path(Some("my session/2")).as_deref(),
            Some("/cache/tab-groups-my_session_2")
        );
        assert_eq!(store::path(None), None);
    }

    #[test]
    fn the_file_is_replaced_whole() {
        let folder = format!("/tmp/zjstatus-test-{}", uuid::Uuid::new_v4());
        std::fs::create_dir(&folder).unwrap();
        let path = format!("{folder}/tab-groups-main");
        let mut state = CollapseState::default();
        state.set("api", true);

        store::write(&path, &state);
        state.set("web", true);
        store::write(&path, &state);

        assert_eq!(store::read(&path), state);
        let files: Vec<_> = std::fs::read_dir(&folder).unwrap().collect();
        assert_eq!(files.len(), 1, "no temporary files are left");
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn groups_use_the_default_unless_chosen() {
        let mut state = CollapseState::default();
        assert!(!state.is_collapsed("api", false));
        assert!(state.is_collapsed("api", true));

        state.set("api", false);
        assert!(!state.is_collapsed("api", true));

        state.set_all(true);
        assert!(state.is_collapsed("api", false));
        assert!(state.groups.is_empty());
    }

    #[test]
    fn parses_pipe_commands() {
        assert_eq!(
            parse_command("zjstatus::groups::toggle"),
            Some(Command::Toggle(None))
        );
        assert_eq!(
            parse_command("zjstatus::groups::toggle::api"),
            Some(Command::Toggle(Some("api".to_owned()))),
        );
        assert_eq!(
            parse_command("zjstatus::groups::collapse::a::b"),
            Some(Command::Collapse(Some("a::b".to_owned()))),
        );
        assert_eq!(
            parse_command("zjstatus::groups::expand"),
            Some(Command::Expand(None))
        );
        assert_eq!(
            parse_command("zjstatus::groups::collapse_all"),
            Some(Command::CollapseAll)
        );
        assert_eq!(
            parse_command("zjstatus::groups::expand_all\n"),
            Some(Command::ExpandAll)
        );
        assert_eq!(
            parse_command("zjstatus::groups::toggle_all"),
            Some(Command::ToggleAll)
        );
        assert_eq!(parse_command("zjstatus::groups::nope"), None);
        assert_eq!(parse_command("zjstatus::rerun::groups"), None);
        assert_eq!(parse_command("zjstatus::groups"), None);
    }

    #[test]
    fn applies_commands_to_the_named_or_active_group() {
        let config = config();
        let mut state = CollapseState::default();
        let in_api = Shown {
            groups: names(&["api", "web"]),
            active: Some("api".to_owned()),
        };
        let elsewhere = Shown {
            groups: names(&["api", "web"]),
            active: None,
        };

        assert!(apply(&mut state, &Command::Toggle(None), &in_api, &config));
        assert!(state.is_collapsed("api", false));
        assert!(apply(
            &mut state,
            &Command::Toggle(Some("api".into())),
            &elsewhere,
            &config
        ));
        assert!(!state.is_collapsed("api", false));
        assert!(!apply(&mut state, &Command::Expand(None), &in_api, &config));
        assert!(!apply(
            &mut state,
            &Command::Toggle(None),
            &elsewhere,
            &config
        ));
        assert!(apply(
            &mut state,
            &Command::CollapseAll,
            &elsewhere,
            &config
        ));
        assert!(state.is_collapsed("web", false));
    }

    #[test]
    fn toggle_all_collapses_everything_unless_all_are_collapsed() {
        let config = config();
        let shown = Shown {
            groups: names(&["api", "web"]),
            active: None,
        };
        let mut state = CollapseState::default();
        state.set("api", true);

        assert!(apply(&mut state, &Command::ToggleAll, &shown, &config));
        assert!(state.is_collapsed("api", false) && state.is_collapsed("web", false));
        assert!(apply(&mut state, &Command::ToggleAll, &shown, &config));
        assert!(!state.is_collapsed("api", false) && !state.is_collapsed("web", false));
        assert!(
            !apply(&mut state, &Command::ToggleAll, &Shown::default(), &config),
            "no groups"
        );
    }

    #[test]
    fn finds_its_own_tab_from_the_plugin_pane() {
        use std::collections::HashMap;
        use zellij_tile::prelude::PaneInfo;

        let plugin = |id| PaneInfo {
            id,
            is_plugin: true,
            ..PaneInfo::default()
        };
        let terminal = PaneInfo {
            id: 7,
            ..PaneInfo::default()
        };
        let panes = PaneManifest {
            panes: HashMap::from([
                (0, vec![plugin(1), terminal.clone()]),
                (1, vec![plugin(7), terminal]),
            ]),
        };

        assert_eq!(own_tab_position(&panes, 7), Some(1));
        assert_eq!(own_tab_position(&panes, 9), None);
    }

    /// The state an instance sees: `names` as tabs, with its own plugin pane
    /// (id 99) in the tab at `own`.
    fn seen_from(names: &[&str], own: usize) -> ZellijState {
        use std::collections::HashMap;
        use zellij_tile::prelude::{ModeInfo, PaneInfo};

        let bar = PaneInfo {
            id: 99,
            is_plugin: true,
            ..PaneInfo::default()
        };
        ZellijState {
            tabs: tabs(names),
            panes: PaneManifest {
                panes: HashMap::from([(own, vec![bar])]),
            },
            mode: ModeInfo {
                session_name: Some("main".to_owned()),
                ..ModeInfo::default()
            },
            ..ZellijState::default()
        }
    }

    fn controller() -> Controller {
        Controller {
            config: Some(config()),
            plugin_id: Some(99),
            ..Controller::default()
        }
    }

    /// Regression: a tab opened in the background moved only once it had been
    /// shown, as only its own bar moved it, and that bar gets no tab updates
    /// until then.
    #[test]
    fn the_active_tabs_bar_moves_any_tab_to_its_group() {
        let to_group = Move {
            tab_id: 13,
            position: 1,
        };
        // The tabs, and the bar's tab before and after the move.
        for (names, own, own_after) in [
            (["api: a*", "x", "y", "api: b"], 0, 0),
            (["api: a", "x", "y", "api: b*"], 3, 1),
        ] {
            let mut controller = controller();
            let state = seen_from(&names, own);

            assert_eq!(
                controller.keep_together(&state),
                Some(to_group.clone()),
                "{names:?}"
            );
            assert_eq!(
                controller.keep_together(&state),
                None,
                "not repeated in the same order"
            );

            let mut moved = seen_from(&[names[0], names[3], names[1], names[2]], own_after);
            for (tab, id) in moved.tabs.iter_mut().zip([10, 13, 11, 12]) {
                tab.tab_id = id;
            }
            assert_eq!(controller.keep_together(&moved), None, "together now");
        }
    }

    #[test]
    fn bars_of_inactive_tabs_move_nothing() {
        for own in [1, 2] {
            let state = seen_from(&["api: a*", "x", "api: b"], own);

            assert_eq!(controller().keep_together(&state), None, "tab {own}");
        }
    }

    #[test]
    fn nothing_moves_while_a_tab_is_renamed() {
        let mut state = seen_from(&["api: a*", "x", "api: b"], 0);
        state.mode.mode = InputMode::RenameTab;

        assert_eq!(controller().keep_together(&state), None);
    }

    #[test]
    fn a_move_that_did_nothing_is_retried_only_after_the_tabs_change() {
        let mut controller = controller();
        let state = seen_from(&["api: a*", "x", "api: b"], 0);
        let to_group = Move {
            tab_id: 12,
            position: 1,
        };

        assert_eq!(controller.keep_together(&state), Some(to_group.clone()));
        assert_eq!(controller.keep_together(&state), None);

        let mut changed = seen_from(&["api: a*", "x", "api: b", "new"], 0);
        changed.tabs[3].tab_id = 42;
        assert_eq!(controller.keep_together(&changed), Some(to_group));
    }
}
