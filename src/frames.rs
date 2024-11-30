use zellij_tile::prelude::*;

#[tracing::instrument(skip_all)]
pub fn hide_frames_conditionally(
    cfg_hide_frames_for_single_pane: bool,
    cfg_hide_frames_except_for_search: bool,
    cfg_hide_frames_except_for_fullscreen: bool,
    tabs: &[TabInfo],
    pane_info: &PaneManifest,
    mode_info: &ModeInfo,
    plugin_pane_id: PluginIds,
) {
    if !cfg_hide_frames_for_single_pane
        && !cfg_hide_frames_except_for_search
        && !cfg_hide_frames_except_for_fullscreen
    {
        return;
    }

    let panes = match get_current_panes(tabs, pane_info) {
        Some(panes) => panes,
        None => return,
    };

    // check if we are running for the current tab since one plugin will run for
    // each tab. If we do not prevent execution, the screen will start to flicker
    // 'cause every plugin will try to toggle the frames
    if !is_plugin_for_current_tab(&panes, plugin_pane_id) {
        return;
    }

    let panes: Vec<&PaneInfo> = panes
        .iter()
        .filter(|p| !p.is_plugin && !p.is_floating)
        .collect();

    let frame_enabled = panes.iter().any(|p| p.pane_content_x - p.pane_x > 0);

    let frames_for_search =
        cfg_hide_frames_except_for_search && should_show_frames_for_search(mode_info);
    let frames_for_fullscreen =
        cfg_hide_frames_except_for_fullscreen && should_show_frames_for_fullscreen(&panes);
    let frames_for_single_pane =
        cfg_hide_frames_for_single_pane && should_show_frames_for_multiple_panes(mode_info, &panes);

    if (frames_for_search || frames_for_fullscreen || frames_for_single_pane) && !frame_enabled {
        toggle_pane_frames();
    }

    if (!frames_for_search && !frames_for_fullscreen && !frames_for_single_pane) && frame_enabled {
        toggle_pane_frames();
    }
}

pub fn should_show_frames_for_search(mode_info: &ModeInfo) -> bool {
    mode_info.mode == InputMode::EnterSearch
}

pub fn should_show_frames_for_fullscreen(panes: &[&PaneInfo]) -> bool {
    let active_pane = match panes.iter().find(|p| p.is_focused) {
        Some(p) => p,
        None => return false,
    };

    active_pane.is_fullscreen
}

#[tracing::instrument(skip_all)]
pub fn should_show_frames_for_multiple_panes(mode_info: &ModeInfo, panes: &[&PaneInfo]) -> bool {
    tracing::debug!("mode: {:?}", mode_info.mode);
    if mode_info.mode == InputMode::RenamePane
        || mode_info.mode == InputMode::Search
        || mode_info.mode == InputMode::EnterSearch
    {
        return true;
    }

    panes.len() > 1
}

fn is_plugin_for_current_tab(panes: &[PaneInfo], plugin_pane_id: PluginIds) -> bool {
    panes
        .iter()
        .any(|p| p.is_plugin && p.id == plugin_pane_id.plugin_id)
}

fn get_current_panes(tabs: &[TabInfo], pane_info: &PaneManifest) -> Option<Vec<PaneInfo>> {
    let active_tab = tabs.iter().find(|t| t.active);
    let active_tab = active_tab.as_ref()?;

    pane_info.panes.get(&active_tab.position).cloned()
}
