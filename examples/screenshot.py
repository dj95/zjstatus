"""Turns the output of `cargo run --example screenshot` (caption lines starting
with `# ` and bar lines with ANSI styles) into an HTML page. See screenshot.sh."""

import html
import re
import sys

# Catppuccin Mocha: the terminal's background and text, and the 16 ANSI colours.
BACKGROUND, TEXT = "#1e1e2e", "#cdd6f4"
ANSI = ["#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#bac2de",
        "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#a6adc8"]
SGR = re.compile(r"\x1b\[([0-9;]*)m")


def bar_html(line):
    style = {}
    output = []
    position = 0
    for match in list(SGR.finditer(line)) + [None]:
        end = match.start() if match else len(line)
        text = line[position:end]
        if text:
            css = ";".join(f"{key}:{value}" for key, value in style.items())
            output.append(f'<span style="{css}">{html.escape(text)}</span>')
        if not match:
            break
        position = match.end()
        codes = [int(code) if code else 0 for code in match.group(1).split(";")]
        while codes:
            code = codes.pop(0)
            if code == 0:
                style.clear()
            elif code == 1:
                style["font-weight"] = "bold"
            elif code == 2:
                style["opacity"] = "0.6"
            elif code == 3:
                style["font-style"] = "italic"
            elif code == 4:
                style["text-decoration"] = "underline"
            elif code in (38, 48) and codes[:1] == [2]:
                red, green, blue = codes[1:4]
                del codes[:4]
                key = "color" if code == 38 else "background"
                style[key] = f"rgb({red},{green},{blue})"
            elif code in (38, 48) and codes[:1] == [5]:
                index = codes[1]
                del codes[:2]
                if index < 16:
                    style["color" if code == 38 else "background"] = ANSI[index]
            elif 30 <= code <= 37 or 90 <= code <= 97:
                style["color"] = ANSI[code - 30 if code < 90 else code - 82]
            elif 40 <= code <= 47 or 100 <= code <= 107:
                style["background"] = ANSI[code - 40 if code < 100 else code - 92]
    return "".join(output)


def main():
    scenes = []
    for line in sys.stdin.read().splitlines():
        if line.startswith("# "):
            scenes.append(f'<div class="caption">{html.escape(line[2:])}</div>')
        elif line:
            scenes.append(f'<div class="bar">{bar_html(line)}</div>')
    print(f"""<!doctype html>
<html><head><meta charset="utf-8"><style>
  body {{ margin: 0; background: #11111b; }}
  #frame {{ display: inline-block; padding: 16px; }}
  .caption {{ color: #a6adc8; font: 13px -apple-system, "Helvetica Neue", sans-serif; margin: 14px 0 6px 2px; }}
  .caption:first-child {{ margin-top: 0; }}
  .bar {{ background: {BACKGROUND}; color: {TEXT}; white-space: pre; border-radius: 6px; padding: 0 6px;
         font: 15px/30px "JetBrainsMono Nerd Font Mono", "JetBrainsMono NFM", monospace; }}
</style></head><body><div id="frame">
{chr(10).join(scenes)}
</div></body></html>""")


if __name__ == "__main__":
    main()
