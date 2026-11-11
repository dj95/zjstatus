//! Renders the bar of `examples/groups.kdl` with the plugin's own code, for
//! the screenshot in FORK.md: one `# caption` line and one bar line (with ANSI
//! styles) per scene. `examples/screenshot.sh` turns them into an image.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use kdl::{KdlDocument, KdlNode};
use zellij_tile::prelude::*;
use zjstatus::{
    config::{ModuleConfig, ZellijState},
    widgets::{attention::AttentionWidget, mode::ModeWidget, tabs::TabsWidget, widget::Widget},
};

const LAYOUT: &str = include_str!("groups.kdl");
const COLUMNS: usize = 100;

use AttentionStatus::{Done, Failed, NeedsYou, Working};

/// A tab: its name, whether it is active, and the statuses of its panes.
type Tab<'a> = (&'a str, bool, &'a [AttentionStatus]);

fn main() {
    let config = plugin_config();

    let tabs = |active: &str| -> Vec<Tab> {
        [
            ("notes", &[][..]),
            ("api: auth", &[Working][..]),
            ("api: billing", &[NeedsYou][..]),
            ("api: search", &[][..]),
            ("web: ui", &[Done][..]),
            ("web: docs", &[Working][..]),
            ("web: e2e", &[Failed][..]),
        ]
        .into_iter()
        .map(|(name, statuses)| (name, name == active, statuses))
        .collect()
    };

    let scenes = [
        (
            "api expanded, web collapsed",
            tabs("api: auth"),
            vec!["web"],
        ),
        ("all groups collapsed", tabs("notes"), vec!["api", "web"]),
        (
            "the active tab in a collapsed group",
            tabs("web: docs"),
            vec!["api", "web"],
        ),
    ];

    for (caption, tabs, collapsed) in scenes {
        println!("# {caption}");
        println!("{}", render(&config, &tabs, &collapsed));
    }
}

fn render(config: &BTreeMap<String, String>, tabs: &[Tab], collapsed: &[&str]) -> String {
    let mut state = ZellijState {
        cols: COLUMNS,
        mode: ModeInfo {
            mode: InputMode::Normal,
            ..ModeInfo::default()
        },
        ..ZellijState::default()
    };
    let mut panes = HashMap::new();
    for (position, (name, active, statuses)) in tabs.iter().enumerate() {
        let mut counts = AttentionCounts::default();
        let tab_panes: Vec<PaneInfo> = statuses
            .iter()
            .enumerate()
            .map(|(index, status)| {
                let attention = PaneAttention {
                    status: *status,
                    source: Some(AttentionSource::ProgramStatus),
                    ..PaneAttention::default()
                };
                counts.add(&attention);
                PaneInfo {
                    id: (position * 10 + index) as u32,
                    attention,
                    ..PaneInfo::default()
                }
            })
            .collect();
        panes.insert(position, tab_panes);
        state.tabs.push(TabInfo {
            position,
            tab_id: position,
            name: name.to_string(),
            active: *active,
            attention: TabAttention {
                counts,
                ..TabAttention::default()
            },
            ..TabInfo::default()
        });
    }
    state.panes = PaneManifest { panes };
    for group in collapsed {
        state.tab_groups.collapse.set(group, true);
    }

    let widgets: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::from([
        (
            "mode".to_owned(),
            Arc::new(ModeWidget::new(config)) as Arc<dyn Widget>,
        ),
        ("tabs".to_owned(), Arc::new(TabsWidget::new(config))),
        (
            "attention".to_owned(),
            Arc::new(AttentionWidget::new(config)),
        ),
    ]);
    ModuleConfig::new(config)
        .expect("the example config is valid")
        .render_bar(state, widgets)
}

/// The settings of the zjstatus plugin in the layout.
fn plugin_config() -> BTreeMap<String, String> {
    let layout: KdlDocument = LAYOUT.parse().expect("groups.kdl is valid KDL");
    let plugin = find(&layout, "plugin").expect("groups.kdl has a plugin");
    plugin
        .children()
        .into_iter()
        .flat_map(|settings| settings.nodes())
        .filter_map(|setting| {
            let value = setting.entries().first()?.value().as_string()?;
            Some((setting.name().value().to_owned(), value.to_owned()))
        })
        .collect()
}

fn find<'a>(document: &'a KdlDocument, name: &str) -> Option<&'a KdlNode> {
    document.nodes().iter().find_map(|node| {
        if node.name().value() == name {
            return Some(node);
        }
        node.children().and_then(|children| find(children, name))
    })
}
