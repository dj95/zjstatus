use zellij_tile::prelude::*;

pub fn hide_frames_on_single_pane(tabs: Vec<TabInfo>, pane_info: PaneManifest) {
    let active_tab = tabs.into_iter().find(|t| t.active);
    if active_tab.is_none() {
        return;
    }

    let active_tab_id = active_tab.unwrap().position;

    let panes = pane_info.panes.get(&active_tab_id);
    if panes.is_none() {
        return;
    }

    let panes = panes.unwrap();
    let panes: Vec<&PaneInfo> = panes.iter().filter(|p| !p.is_plugin).collect();

    if panes.len() == 1 && panes.first().unwrap().pane_content_x == 1 {
        toggle_pane_frames();
    }

    if panes.len() > 1 && panes.first().unwrap().pane_content_x == 0 {
        toggle_pane_frames();
    }
}
