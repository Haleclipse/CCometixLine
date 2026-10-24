# CCometixLine

[English](README.md) | [中文](README.zh.md)

A fast, composable status line for Claude Code, written in Rust, with a full-screen configurator.

![Language:Rust](https://img.shields.io/static/v1?label=Language&message=Rust&color=orange&style=flat-square)
![License:MIT](https://img.shields.io/static/v1?label=License&message=MIT&color=blue&style=flat-square)

![CCometixLine in Claude Code](assets/hero.webp)

## Highlights

- **Your session at a glance**: model and effort, directory, git, context use, prompt cache countdown and hit ratio, the 5-hour and weekly limits with your pace against the clock, cost, PR and more: 30+ metrics in all.
- **Any metric, any form**: each item can be text, a progress bar, a gauge, a sparkline or a pace bar, with three text templates for when room runs short.
- **Fits the terminal**: lines wrap between items when the terminal is narrow; a right-hand group stays flush right.
- **Looks**: four skins (plain, bracket, pill, powerline), 13 palettes that follow your terminal's colors or bring their own, Nerd Font, Unicode or ASCII glyphs, and color depth from truecolor down to none.
- **Eight presets** to start from, and your own saved alongside them.
- **A configurator** that edits everything with a live preview of sample sessions, and points Claude Code at ccline for you.
- **Accurate for third-party models**: a model registry fixes names and context windows Claude Code can't know.
- **Background tasks**: the same binary draws the line under each running subagent.
- **Light**: a render costs a few milliseconds over process start; git runs in the background, never on the render path.

## Install

```bash
npm install -g @cometix/ccline
```

<details>
<summary>yarn, pnpm, the npm mirror</summary>

```bash
yarn global add @cometix/ccline
pnpm add -g @cometix/ccline
npm install -g @cometix/ccline --registry https://registry.npmmirror.com
```

</details>

The package puts the binary at `~/.claude/ccline/ccline` and the `ccline` command on your PATH. To update later: `npm update -g @cometix/ccline`.

<details>
<summary>Manual download</summary>

Download the archive for your platform from [Releases](https://github.com/Haleclipse/CCometixLine/releases) and put the binary at `~/.claude/ccline/ccline` (`ccline.exe` on Windows).

| Platform | Archive |
|---|---|
| Linux x64 (glibc 2.35+) | `ccline-linux-x64.tar.gz` |
| Linux x64, static | `ccline-linux-x64-static.tar.gz` |
| Linux arm64 | `ccline-linux-arm64.tar.gz` |
| Linux arm64, static | `ccline-linux-arm64-static.tar.gz` |
| macOS Intel | `ccline-macos-x64.tar.gz` |
| macOS Apple Silicon | `ccline-macos-arm64.tar.gz` |
| Windows x64 | `ccline-windows-x64.zip` |

```bash
mkdir -p ~/.claude/ccline
tar -xzf ccline-macos-arm64.tar.gz
mv ccline ~/.claude/ccline/ && chmod +x ~/.claude/ccline/ccline
```

</details>

<details>
<summary>Build from source</summary>

```bash
git clone https://github.com/Haleclipse/CCometixLine.git
cd CCometixLine
cargo build --release
mkdir -p ~/.claude/ccline
cp target/release/ccometixline ~/.claude/ccline/ccline
```

</details>

## Set up Claude Code

Run `ccline` in a terminal. The first run walks you through checking your font, picking a preset and pointing Claude Code at ccline; the **Claude Code** page shows the change to `settings.json` before writing it, and keeps a backup.

Or edit `~/.claude/settings.json` yourself:

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/ccline/ccline",
    "padding": 0
  },
  "subagentStatusLine": {
    "type": "command",
    "command": "~/.claude/ccline/ccline"
  }
}
```

`subagentStatusLine` is optional: it draws ccline's line under each background task. `~` works on Windows too (Claude Code 2.1.47+); with a global npm install, `"command": "ccline"` also works.

## Presets

![Presets](assets/presets.webp)

| Preset | Lines | Look | What it shows |
|---|---|---|---|
| `cometix` (default) | 1 | plain, terminal colors | Classic look: model, directory, git, context and cache |
| `minimal` | 1 | plain, no icons | Model, context, cache and cost |
| `dashboard` | 3 | pill, Catppuccin Mocha | Identity and git; context, its trend, 5-hour pace and cache; code activity |
| `budget` | 2 | plain, Tokyo Night | 5-hour and weekly limits with pace and run-out, spend rates; context, compaction and cache |
| `cache-aware` | 2 | plain, Rosé Pine | The usual line; cache countdown bar, hit ratio, misses and their cause |
| `focus` | 2 | plain, Unicode | A wide context gauge, growth per turn, room before compaction, cache |
| `zen` | 1 | ASCII, no color | Works in any terminal |
| `powerline` | 1 | powerline, Tokyo Night | Powerline segments with a 5-hour gauge |

Apply one in the configurator: all of it, only its layout, or only its look. Save your own setup as a preset with `N` on the Presets page; it is a file in `~/.claude/ccline/presets/`.

## The configurator

Run `ccline` (or `ccline -c`). Six pages, switched with `1`–`6`; every change shows at once in the preview bar, drawn by the same code as the real status line.

| Page | What it does |
|---|---|
| 1 Home | Checks your setup (Claude Code version, its status line settings, `config.toml`, `models.toml`, updates) with a key to fix each problem, and shows your line with your last real session |
| 2 Presets | The gallery, each card rendered live |
| 3 Layout | Pick metrics from the library, arrange them on lines and sides, and tune the selected item: form, bar glyphs and cells, templates, icon, colors, thresholds |
| 4 Style | Skin, palette, glyphs, color depth, fill, and color overrides on top of the palette |
| 5 Models | The model registry: unrecognized ids from recent sessions, your rules with hit counts, and a tester |
| 6 Claude Code | `statusLine`, `subagentStatusLine` and `refreshInterval`, update checks, the companion mod |

![Layout page](assets/tui-layout.webp)

The icon picker covers all 10,617 icons of Nerd Fonts 3.5.1, searchable by name, class (`md-`, `fa-`, `cod-` …), codepoint or a pasted glyph.

![Icon picker](assets/tui-icons.webp)

Keys everywhere: `^S` save, `^Z` / `^Y` undo and redo, `^P` command palette, `[` `]` sample session, `-` `+` preview width, `{` color depth, `}` glyphs, `?` help, `q` quit.

## Metrics

| Category | Metrics |
|---|---|
| Identity | `model` (with reasoning effort), `output_style`, `vim`, `agent` |
| Session | `session_name`, `duration` |
| Context | `context`, `context.trend`, `context.to_compact`, `context.breakdown`¹ |
| Quota | `quota.5h`, `quota.7d`, `quota.spend`, `quota.eta` |
| Cost | `cost`, `cost.rate` (per hour), `cost.today` (all sessions) |
| Cache | `cache` (time until cold, hit ratio, misses), `cache.cold_cost` |
| Code | `dir`, `git`, `repo`, `pr`, `worktree`, `lines` |
| Activity, timing | `activity.tools`¹, `activity.agents`¹, `timing.ttft`¹, `timing.tps`¹ |
| System | `cc_version`, `update` |
| Custom | `cmd`: the first line of any shell command, cached and run in the background |

¹ Needs the companion mod and Claude Code 2.1.287+. `cache` and `quota.spend` need Claude Code 2.1.251+. The configurator marks what your Claude Code can't show and won't let you add it.

Items show only when they have something to say: the cache when Claude Code reports one, the update when a newer ccline is out, `output_style` unless it is the default.

`quota.5h` and `quota.7d` draw a two-row pace bar by default: the upper row is what you have used, the lower row how much of the window has passed. `⇡8%` means you are 8 points ahead of the clock, `⇣` behind, `±0%` on pace.

Background task lines have their own metrics: `task.label`, `task.name`, `task.status`, `task.elapsed` and `task.trend`, next to session metrics such as `model` and `context`.

## Configuration

Everything lives in `~/.claude/ccline/` (`CCLINE_HOME` moves it). The configurator writes `config.toml`; you can edit it by hand too:

```toml
preset  = "cometix"
palette = "terminal"
skin    = "plain"
glyphs  = "nerd"
separator = " | "

[[line]]
left = [
  "model",
  { metric = "dir", icon_color = "bright_yellow" },
  "git",
  { metric = "context", widget = "text", format = "{pct}%[ · {tokens} tokens]" },
  "cache",
]
right = ["update"]

[[line]]
left = [{ metric = "quota.5h", cells = 10 }, { metric = "quota.7d", cells = 10 }]
right = [{ metric = "cmd", options = { run = "kubectl config current-context", cache_s = 30 } }]
```

Each `[[line]]` has a `left` and a `right` group; the right one is flush right. An item is a metric id, or a table:

| Key | Meaning |
|---|---|
| `metric` | Metric id |
| `widget` | `text`, `bar`, `gauge`, `sparkline` or `pace` |
| `bar`, `cells` | Bar glyphs (`blocks`, `eighths`, `shapes`, `dots`, `braille`, `ascii`) and length |
| `format`, `compact`, `minimal` | Text templates: `{field}` inserts a value, `[ … ]` drops its contents when a field inside is empty |
| `icon` | Icon override; `""` for none |
| `color`, `fg`, `icon_color` | A palette token (`accent`, `warn`, `cache` …), a color name (`bright_yellow`), `#rrggbb`, `256:n` or `16:n` |
| `levels`, `coloring` | Warning thresholds such as `[[50, "warn"], [80, "crit"]]`; `steps` or `gradient` |
| `options` | Metric options, such as `dir`'s `style` or `cmd`'s `run` |

Palettes: `terminal` (follows your terminal theme), `catppuccin-mocha`, `catppuccin-latte`, `dracula`, `nord`, `tokyo-night`, `rose-pine`, `gruvbox`, `anthropic-clay`, and v1's powerline colors as `powerline-dark`, `powerline-light`, `powerline-rose-pine` and `powerline-tokyo-night`. Your own go in `palettes/*.toml`; `[palette_overrides]` changes single tokens.

A config that fails to parse never blanks the status line: ccline draws the default preset and adds `! config` at the end.

### Third-party models

Claude Code assumes a 200k window for models it doesn't know, so the context percentage of a third-party model can be wrong. `models.toml` sets the right name and window:

```toml
[[models]]
pattern = "kimi-k2"
display_name = "Kimi K2"
context_limit = 256000
match = "contains"          # contains, prefix, exact or regex

[[context_modifiers]]
pattern = "[1m]"
display_suffix = " 1M"
context_limit = 1000000
```

The Models page lists the ids it has seen that nothing recognizes, so you can add a rule with one key.

## Companion mod

On Claude Code 2.1.287+ an optional mod passes ccline what the status line input lacks: time to first token, output speed, running tools and agents, what fills the context, and the exact auto-compact point. It draws nothing itself.

```bash
ccline mod install     # or from the Claude Code page
ccline mod status
ccline mod uninstall
```

Live values need Claude Code to redraw the status line on a timer: set `refreshInterval` to 2 seconds on the Claude Code page.

## Commands

| Command | |
|---|---|
| `ccline` | Opens the configurator in a terminal; renders the status line when Claude Code pipes it a session |
| `ccline preview` | Renders your config with the built-in sample sessions (`--all`, `-s <name or JSON file>`, `--subagent`, `--list`) |
| `ccline update` | Checks for a newer version now. ccline also checks once a day in the background and shows `update` when one is out; it never downloads anything |
| `ccline mod` | Installs, updates or removes the companion mod |

## Requirements

- Claude Code 2.1.153 or newer; some metrics need newer, see above.
- A [Nerd Font](https://www.nerdfonts.com/) for icons, powerline and pills. Without one, use the `unicode` or `ascii` glyphs: the configurator's font check helps you choose.
- Git, for the `git` metric.

## Upgrading from v1

v2 is a rewrite with a new config format; it doesn't read v1's `config.toml` or themes. Pick a preset in the configurator to start, or `cometix`, the default, which keeps v1's look. `models.toml` carries over unchanged. `--theme` and the Claude Code patcher are gone.

## Development

```bash
cargo build
cargo test             # unit tests and snapshots of every preset in every sample session
cargo build --release
```

## Related projects

- [tweakcc](https://github.com/Piebald-AI/tweakcc): customize Claude Code's themes, thinking verbs and more.

## License

[MIT](LICENSE)

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Haleclipse/CCometixLine&type=Date)](https://star-history.com/#Haleclipse/CCometixLine&Date)
