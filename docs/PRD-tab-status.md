# PRD: Поддержка emoji-статусов табов в zjstatus

## Проблема

Проект [zellij-tab-status](https://github.com/dapi/zellij-tab-status) управляет emoji-статусами табов Zellij через `rename_tab()`. Этот API содержит [баг #3535](https://github.com/zellij-org/zellij/issues/3535) — использует persistent internal index вместо position. ~70% кода плагина (probing FSM, pane-ID якоря, timer-based gap detection) борется с этим багом, а не решает задачу.

**Цель:** перенести рендеринг статусов в zjstatus через placeholder `{status}`, полностью исключив `rename_tab()`. CLI-скрипт `zellij-tab-status` сохраняет тот же интерфейс, но отправляет команды zjstatus вместо отдельного WASM-плагина.

## Компоненты

### 1. zjstatus: placeholder `{status}` и pipe-команды

#### 1.1 State

Новое поле в `ZellijState`:

```rust
pub tab_statuses: HashMap<usize, String>  // tab_index → emoji
```

Статус — свойство **таба**, не pane. CLI отправляет `pane_id` (единственное что доступно через `$ZELLIJ_PANE_ID`), zjstatus при получении pipe-команды маппит `pane_id → tab_index` через `state.panes` (`PaneManifest` — уже есть в `ZellijState`, обновляется через `PaneUpdate` event) и хранит по tab_index.

#### 1.2 Pipe-протокол

Формат: `zjstatus::command::arg1::arg2` (совместим с существующим pipe-протоколом zjstatus).

| Команда | Формат | Описание |
|-|-|-|
| set_status | `zjstatus::set_status::{pane_id}::{emoji}` | Маппит pane_id → tab_index, сохраняет emoji |
| clear_status | `zjstatus::clear_status::{pane_id}` | Маппит pane_id → tab_index, удаляет статус |

**Парсинг emoji:** строго `parts[3]` — один сегмент между разделителями `::`.

**Валидация:**
- `pane_id` не парсится как число → игнорировать, лог в stderr
- `pane_id` не найден в `PaneManifest` → игнорировать, лог в stderr
- Пустой emoji в `set_status` → трактовать как `clear_status`
- `clear_status` для таба без статуса → no-op (idempotent)
- Emoji содержит `:` или `::` → **не поддерживается**, `::` — разделитель протокола, это ограничение by design

**Не входит в v1:** `get_status` — текущий pipe-протокол zjstatus не поддерживает обратный канал (pipe output). Добавим когда появится реальная потребность.

#### 1.3 Placeholder `{status}`

Доступен в tab-шаблонах: `tab_active`, `tab_normal`, `tab_floating`, `tab_sync`, `tab_fullscreen`, `tab_rename`.

**Логика подстановки:**
1. При рендере таба — посмотреть `tab_statuses[tab_index]`
2. Если есть — подставить emoji как есть (без добавления пробела)
3. Если нет — пустая строка

Пробел между `{status}` и `{name}` — ответственность шаблона, не кода. Это позволяет пользователю контролировать форматирование.

#### 1.4 Cleanup

При получении `PaneUpdate` / `TabUpdate` — удалять записи из `tab_statuses` для tab_index, которых больше нет в `TabInfo`. Предотвращает утечку памяти при закрытии табов.

#### 1.5 UpdateEventMask

Ререндер не нужен по маске — `parse_protocol` уже возвращает `should_render = true` при успешной обработке команды (см. `pipe.rs:57-78`). Новые команды `set_status`/`clear_status` будут возвращать `true`, что вызовет ререндер. Маска `{status}` placeholder не требуется — он рендерится в контексте `TabsWidget`, который и так обновляется при ререндере.

#### 1.6 Пример конфигурации

```kdl
tab_active   "#[fg=#89B4FA,bold] {status} {name} "
tab_normal   "#[fg=#6C7086] {status} {name} "
```

Результат: `🤖 my-tab` когда статус установлен, ` my-tab` (лишний пробел) когда нет.

Для избежания лишнего пробела — шаблон без пробела:
```kdl
tab_active   "#[fg=#89B4FA,bold] {status}{name} "
```
Результат: `🤖my-tab` / `my-tab`. Пользователь добавляет пробел в emoji: `zellij-tab-status "🤖 "`.

#### 1.7 Persistence

Статусы живут только в памяти zjstatus. При перезагрузке плагина (clear cache, перезапуск Zellij) — теряются. Это by design: статусы отражают текущее runtime-состояние (AI-агент работает, задача выполнена), а не постоянные метки.

### 2. CLI-скрипт `zellij-tab-status`

Shell-скрипт с **тем же интерфейсом** что и текущий (за исключением убранных команд), отправляющий команды zjstatus вместо отдельного WASM-плагина.

#### 2.1 Интерфейс

```
zellij-tab-status 🤖           # установить emoji
zellij-tab-status --clear, -c  # убрать статус
zellij-tab-status --version, -v
zellij-tab-status --help, -h
```

#### 2.2 Убираемые команды

| Команда | Причина удаления |
|-|-|
| `--get`, `-g` | Нет обратного канала в pipe-протоколе zjstatus |
| `--name`, `-n` | zjstatus не переименовывает табы — имя всегда оригинальное |
| `--set-name`, `-s` | Аналогично — нет rename_tab, нет необходимости |

#### 2.3 Реализация

```bash
# set_status
zellij pipe --name zjstatus -- "zjstatus::set_status::${ZELLIJ_PANE_ID}::${emoji}"

# clear_status
zellij pipe --name zjstatus -- "zjstatus::clear_status::${ZELLIJ_PANE_ID}"
```

#### 2.4 Что исчезает

- WASM-плагин (не нужен)
- `NOT_READY` retry loop (zjstatus уже загружен и имеет PaneManifest)
- Проверка наличия `.wasm` файла
- JSON serialization

## Что не входит в scope

- **`get_status`** — нет обратного канала. Добавим позже при необходимости
- **`set_name` / `get_name`** — zjstatus не управляет именами табов, только рендерит их
- **Probing FSM** — не нужен, нет `rename_tab()`
- **Множественные статусы на одном табе** — статус хранится по tab_index, последняя запись побеждает

## Зависимости

- zjstatus должен быть загружен в layout (уже стандартная практика)
- Пользователь добавляет `{status}` в tab-шаблоны вручную

## Порядок реализации

1. **zjstatus: state + pipe-команды** — `tab_statuses` HashMap, обработка `set_status`/`clear_status` в `pipe.rs`, маппинг pane_id → tab_index
2. **zjstatus: placeholder `{status}`** — подстановка в `TabsWidget` при рендере
3. **zjstatus: cleanup** — очистка `tab_statuses` при удалении табов
4. **CLI-скрипт** — новый `zellij-tab-status`, отправляющий pipe-команды zjstatus
5. **Тесты** — unit-тесты для pipe-команд и подстановки placeholder
