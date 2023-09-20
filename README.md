<h1 align="center">zjstatus 📈🎨</h1>

<p align="center">
  A configurable and themable statusbar for zellij.
  <br><br>
  <a href="https://github.com/dj95/zjstatus/releases">
    <img alt="latest version" src="https://img.shields.io/github/v/tag/dj95/zjstatus.svg?sort=semver" />
  </a>
  <br><br>
  The goal of this statusbar is to provide a highly customizable and extensible statusbar for zellij. Single
  modules can be formatted separately. Due to the widget structure new modules can be created with ease.
</p>

![Screenshot of the statusbar](./assets/demo.png)

<details>
<summary><h3>Examples</h3></summary>
<b><a href="./examples/tmux.kdl">tmux style</a></b>
<img src="./examples/tmux.png" alt="tmux style bar" />
<br>
<b>Demo GIF</b>
<img src="./assets/demo.gif" alt="Demo GIF of zellij with zjstatus" />
</details>

## 🚀 Installation

Download the latest binary in the github releases. Place it somewhere, zellij is able to access it. Then the
plugin can be included by referencing it in a layout file, e.g. in the default layout one.

You could also refer to the plugin guide from zellij, after downloading the binary: [https://zellij.dev/documentation/plugin-loading](https://zellij.dev/documentation/plugin-loading)

Please ensure, that the configuration is correct.

## ❄️ Installation with nix flake

Add this repository to your inputs and then with the following overlay to your packages.
Then you are able to install and refer to it with `pkgs.zjstatus`. When templating the
config file, you can use `${pkgs.zjstatus}/bin/zjstatus.wasm` as the path.

```nix
  inputs = {
    # ...

    zjstatus = {
      url = "github:dj95/zjstatus";
    };
  };


  # define the outputs of this flake - especially the home configurations
  outputs = { self, nixpkgs, zjstatus, ... }@inputs:
  let
    inherit (inputs.nixpkgs.lib) attrValues;

    overlays = with inputs; [
      # ...
      (final: prev: {
        zjstatus = zjstatus.packages.${prev.system}.default;
      })
    ];
```

## ⚙️ Configuration

Configuration can be performed in the layout file, when importing the plugin. Here's a short example.

```kdl

layout {
    pane split_direction="vertical" {
        pane
    }

    pane size=1 borderless=true {
        plugin location="file:target/wasm32-wasi/debug/zjstatus.wasm" {
            format_left  "{mode} #[fg=#89B4FA,bold]{session} {tabs}"
            format_right "{datetime}"
            format_space ""

            hide_frame_for_single_pane "true"

            mode_normal  "#[bg=#89B4FA] "
            mode_tmux    "#[bg=#ffc387] "

            tab_normal   "#[fg=#6C7086] {name} "
            tab_active   "#[fg=#9399B2,bold,italic] {name} "

            datetime        "#[fg=#6C7086,bold] {format} "
            datetime_format "%A, %d %b %Y %H:%M"
            datetime_timezone "Europe/Berlin"
        }
    }
}
```

In order to start using zjstatus you need to specify the widgets you'd like to use under the `format_left` and/or `format_right`
configuration. Formatting can be done with `#[..]`, while widgets and properties are surrounded by `{..}`.
The blank space between the left and the right part can be colored with `format_space`.

The option `hide_frame_for_single_pane` will toggle the pane frames depending on how many panes (not plugin panes) are shown.
This will effectively hide the frame border, when only one pane, like an editor, is shown. Pane frames are toggled as soon
as there is another pane created.

### 🎨 Formatting and theming

Text and modules can be themed with directives in `#[]`. These directives tell zjstatus to print the following
text in the specified format. It will reset the format on any new directives or after rendering a widget.
Options can be combined with a `,`, when they occur in the same bracket.

Possible formatting options are:

| name   | value             | example       | description      |
|--------|-------------------|---------------|------------------|
| fg     | hex or ansi color | `#[fg=#ffffff]` | foreground color |
| bg     | hex or ansi color | `#[bg=#ffffff]` | background color |
| bold   | none              | `#[bold]`       | bold text        |
| italic | none              | `#[italic]`     | italic text      |

### 🧱 Widgets

zjstatus contains the following widgets with their respective config.

#### datetime

**Handle** `{datetime}`

Print the date and/or time by the given format string. Due to the WASM sandbox
the timezone cannot be determined from the system. You can configure it the
with the `datetime_timezone` parameter. Choose the according string from the
chrono documentation: [https://docs.rs/chrono-tz/latest/chrono_tz/enum.Tz.html](https://docs.rs/chrono-tz/latest/chrono_tz/enum.Tz.html)

```kdl
# theme formatting for colors. Datetime output is printed in {format}.
datetime        "#[fg=#6C7086,bold] {format} "

# format of the date. e.g. if you'd like to only show the time
datetime_format "%A, %d %b %Y %H:%M"

# timezone to print
datetime_timezone "Europe/Berlin"
```

#### mode

**Handle** `{mode}`

Indicate the current active mode in zellij. Each mode can be configured individually. If a mode is not configured, it will
fall back to the format of `mode_normal`. The name of the mode can be used in the `{name}` variable.

```kdl
mode_normal        "#[bg=#89B4FA] {name} "
mode_locked        "#[bg=#89B4FA] {name} "
mode_resize        "#[bg=#89B4FA] {name} "
mode_pane          "#[bg=#89B4FA] {name} "
mode_tab           "#[bg=#89B4FA] {name} "
mode_scroll        "#[bg=#89B4FA] {name} "
mode_enter_search  "#[bg=#89B4FA] {name} "
mode_search        "#[bg=#89B4FA] {name} "
mode_rename_tab    "#[bg=#89B4FA] {name} "
mode_rename_pane   "#[bg=#89B4FA] {name} "
mode_session       "#[bg=#89B4FA] {name} "
mode_move          "#[bg=#89B4FA] {name} "
mode_prompt        "#[bg=#89B4FA] {name} "
mode_tmux          "#[bg=#ffc387] {name} "
```

#### session

**Handle** `{session}`

Print the current session name. This module cannot be configured. For formatting, please put the Formatting
sequence right before the handle in `format_left` or `format_right`.

#### swap layout

**Handle** `{swap_layout}`  
**Click behavior** Switch to the next swap layout

Print the active swap layout. This module cannot be configured. For formatting, please put the Formatting
sequence right before the handle in `format_left` or `format_right`.

#### tabs

**Handle** `{tabs}`  
**Click behavior** Navigate to the tab that got clicked

Print a list of current tabs. The name of the tab can be used with `{name}` in the config. The active tab will
default to the normal formatting, if not configured.
With `{index}` the tab position can also be used.

```kdl
# formatting for inactive tabs
tab_normal              "#[fg=#6C7086] {index} :: {name} "
tab_normal_fullscreen   "#[fg=#6C7086] {index} :: {name} [] "
tab_normal_sync         "#[fg=#6C7086] {index} :: {name} <> "

# formatting for the current active tab
tab_active              "#[fg=#9399B2,bold,italic] {name} "
tab_active_fullscreen   "#[fg=#9399B2,bold,italic] {name} [] "
tab_active_sync         "#[fg=#9399B2,bold,italic] {name} <> "
```

## 🚧 Development

Make sure you have rust and the `wasm32-wasi` target installed. If using nix, you could utilize the nix-shell
in this repo for obtaining `cargo` and `rustup`. Then you'll only need to add the target with
`rustup target add wasm32-wasi`.

With the toolchain, simply build `zjstatus` with `cargo build`. Then you are able to run the example configuration
with `zellij -l plugin-dev-workspace.kdl` from the root of the repository.

## 🤝 Contributing

If you are missing features or find some annoying bugs please feel free to submit an issue or a bugfix within a pull request :)

## 📝 License

© 2023 Daniel Jankowski

This project is licensed under the MIT license.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
