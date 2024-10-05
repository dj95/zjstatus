use zellij_tile::prelude::*;

#[tracing::instrument(skip_all)]
pub fn hide_frames_conditionally(
    cfg_hide_frames_for_single_pane: bool,
    cfg_hide_frames_except_for_search: bool,
    tabs: &[TabInfo],
    pane_info: &PaneManifest,
    mode_info: &ModeInfo,
    plugin_pane_id: PluginIds,
) {
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

    if cfg_hide_frames_except_for_search {
        hide_frames_except_for_search(frame_enabled, mode_info);

        return;
    }

    if cfg_hide_frames_for_single_pane {
        hide_frames_on_single_pane(frame_enabled, mode_info, panes);
    }
}

pub fn hide_frames_except_for_search(frame_enabled: bool, mode_info: &ModeInfo) {
    match mode_info.mode {
        InputMode::EnterSearch => {
            if !frame_enabled {
                tracing::debug!("toggle cause search");
                toggle_pane_frames();
            }
        }
        _ => {
            if frame_enabled {
                tracing::debug!("toggle cause not search");
                toggle_pane_frames();
            }
        }
    }
}

#[tracing::instrument(skip_all)]
pub fn hide_frames_on_single_pane(
    frame_enabled: bool,
    mode_info: &ModeInfo,
    panes: Vec<&PaneInfo>,
) {
    tracing::debug!("mode: {:?}", mode_info.mode);
    if mode_info.mode == InputMode::RenamePane
        || mode_info.mode == InputMode::Search
        || mode_info.mode == InputMode::EnterSearch
    {
        if !frame_enabled {
            tracing::debug!("toggle cause of mode");
            toggle_pane_frames();
        }

        return;
    }

    if panes.len() == 1 && frame_enabled {
        tracing::debug!("toggle enabled");
        toggle_pane_frames();
    }

    if panes.len() > 1 && !frame_enabled {
        tracing::debug!("toggle not enabled");
        toggle_pane_frames();
    }
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
