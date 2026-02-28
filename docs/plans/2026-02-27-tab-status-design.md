# Design: Tab emoji status via pipe protocol

**PRD:** `docs/PRD-tab-status.md`
**Approach:** A — `{status}` placeholder в `render_tab`, маппинг в `pipe.rs`

## Точки изменений

### 1. State — `config.rs`

Новое поле в `ZellijState`:

```rust
pub tab_statuses: BTreeMap<usize, String>  // tab_index (position) → emoji
```

`BTreeMap` для консистентности с остальными полями. Инициализация — `BTreeMap::new()` в `zjstatus.rs:load()`.

### 2. Pipe-протокол — `pipe.rs`

Две новых ветки в `process_line` match:

```rust
"set_status" => {
    if parts.len() < 4 { return false; }
    let pane_id: u32 = parts[2].parse()?;  // невалидное → return false
    let emoji = parts[3];                    // строго один сегмент, :: не поддерживается
    if emoji.is_empty() {
        // трактуем как clear
        if let Some(tab_idx) = resolve_tab_index(&state.panes, pane_id) {
            state.tab_statuses.remove(&tab_idx);
        }
    } else {
        if let Some(tab_idx) = resolve_tab_index(&state.panes, pane_id) {
            state.tab_statuses.insert(tab_idx, emoji.to_string());
        }
    }
    should_render = true;
}
"clear_status" => {
    let pane_id: u32 = parts[2].parse()?;
    if let Some(tab_idx) = resolve_tab_index(&state.panes, pane_id) {
        state.tab_statuses.remove(&tab_idx);  // idempotent
    }
    should_render = true;
}
```

Новая функция:

```rust
fn resolve_tab_index(panes: &PaneManifest, pane_id: u32) -> Option<usize> {
    for (tab_index, pane_list) in &panes.panes {
        if pane_list.iter().any(|p| p.id == pane_id) {
            return Some(*tab_index);
        }
    }
    eprintln!("[zjstatus] pane_id {} not found in PaneManifest", pane_id);
    None
}
```

### 3. Рендер — `tabs.rs`

В `render_tab()` — добавить `tab_statuses: &BTreeMap<usize, String>` в сигнатуру (или передавать `&ZellijState`). Подстановка рядом с `{name}` / `{index}`:

```rust
if content.contains("{status}") {
    let status = tab_statuses
        .get(&tab.position)
        .map(|s| s.as_str())
        .unwrap_or("");
    content = content.replace("{status}", status);
}
```

Обновить вызовы `render_tab` в `process()` и `process_click()` — передавать `state.tab_statuses`.

Проблема: `render_tab` сейчас принимает `(tab, panes, mode)`, а не весь state. Варианты:
- **(A)** Добавить `tab_statuses` как отдельный параметр — минимальное изменение
- **(B)** Передавать `&ZellijState` целиком — проще для будущих расширений

Выбрано: **(A)** — добавить один параметр, не ломать существующую сигнатуру больше необходимого.

### 4. Cleanup — `zjstatus.rs`

В `handle_event(Event::TabUpdate)`, после `self.state.tabs = tab_info`:

```rust
let valid_positions: std::collections::BTreeSet<usize> =
    self.state.tabs.iter().map(|t| t.position).collect();
self.state.tab_statuses.retain(|pos, _| valid_positions.contains(pos));
```

### 5. Тесты

**pipe.rs:**
- `set_status` с валидным pane_id → статус появляется в `tab_statuses`
- `set_status` с невалидным pane_id (не число) → false, state не изменён
- `set_status` с несуществующим pane_id → state не изменён
- `set_status` с пустым emoji → удаляет статус (= clear)
- `clear_status` → удаляет статус
- `clear_status` для таба без статуса → no-op
- `resolve_tab_index` — маппинг pane_id → tab_index

**tabs.rs:**
- `render_tab` с `{status}` placeholder и установленным статусом
- `render_tab` с `{status}` placeholder без статуса → пустая строка

### 6. CLI-скрипт

Отдельный shell-скрипт `scripts/zellij-tab-status`:

```bash
zellij pipe --name zjstatus -- "zjstatus::set_status::${ZELLIJ_PANE_ID}::${emoji}"
zellij pipe --name zjstatus -- "zjstatus::clear_status::${ZELLIJ_PANE_ID}"
```

Интерфейс: `zellij-tab-status <emoji>`, `zellij-tab-status --clear`, `--version`, `--help`.

## Не входит в v1

- `get_status` — нет обратного канала в pipe-протоколе
- `set_name` / `get_name` — zjstatus не управляет именами табов
