# @cometix/ccline

A fast, composable status line for Claude Code, written in Rust, with a full-screen configurator.

![CCometixLine in Claude Code](https://raw.githubusercontent.com/Haleclipse/CCometixLine/master/assets/hero.webp)

- Model and effort, directory, git, context use, prompt cache countdown and hit ratio, 5-hour and weekly limits with your pace, cost, PR and more: 30+ metrics
- Text, bars, gauges, sparklines and pace bars, on as many lines as you like, wrapping to fit the terminal
- Four skins, 13 palettes, Nerd Font, Unicode or ASCII glyphs, and eight presets to start from
- A configurator with a live preview that also points Claude Code at ccline for you

## Install

```bash
npm install -g @cometix/ccline
```

In China, the npm mirror is faster:

```bash
npm install -g @cometix/ccline --registry https://registry.npmmirror.com
```

The binary goes to `~/.claude/ccline/ccline`, and `ccline` is on your PATH.

## Set up

Run `ccline` in a terminal: the first run picks a preset and connects Claude Code. Or add this to `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/ccline/ccline",
    "padding": 0
  }
}
```

Requires Claude Code 2.1.153 or newer. Update with `npm update -g @cometix/ccline`.

## More

- Full documentation: https://github.com/Haleclipse/CCometixLine
- 中文说明: https://github.com/Haleclipse/CCometixLine/blob/master/README.zh.md
- Issues: https://github.com/Haleclipse/CCometixLine/issues
- License: MIT
