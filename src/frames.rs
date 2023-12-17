use zellij_tile::prelude::*;

pub fn hide_frames_on_single_pane(
    tabs: Vec<TabInfo>,
    pane_info: PaneManifest,
    plugin_pane_id: PluginIds,
) {
    let panes = match get_current_panes(&tabs, &pane_info) {
        Some(panes) => panes,
        None => return,
    };

    // check if we are running for the current tab since one plugin will run for
    // each tab. If we do not prevent execution, the screen will start to flicker
    // 'cause every plugin will try to toggle the frames
    if !is_plugin_for_current_tab(&panes, plugin_pane_id) {
        return;
    }

    let panes: Vec<&PaneInfo> = panes.iter().filter(|p| !p.is_plugin).collect();

    let first_pane = match panes.first() {
        Some(fp) => fp,
        None => return,
    };

    // frame is enabled, when content does no start at [0, 0]. With default frames
    // it's [1, 1]
    let frame_enabled = first_pane.pane_content_x > 0;

    if panes.len() == 1 && frame_enabled {
        toggle_pane_frames();
    }

    if panes.len() > 1 && !frame_enabled {
        toggle_pane_frames();
    }
}

fn is_plugin_for_current_tab(panes: &[PaneInfo], plugin_pane_id: PluginIds) -> bool {
    let plugin_pane = panes
        .iter()
        .find(|p| p.is_plugin && p.id == plugin_pane_id.plugin_id);

    plugin_pane.is_some()
}

fn get_current_panes(tabs: &[TabInfo], pane_info: &PaneManifest) -> Option<Vec<PaneInfo>> {
    let active_tab = tabs.iter().find(|t| t.active);
    let active_tab = active_tab.as_ref()?;

    pane_info
        .panes
        .get(&active_tab.position)
        .cloned()
}
