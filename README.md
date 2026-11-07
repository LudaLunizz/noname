<p align="center"><img src="assets/demo.gif" width="860"></p>

# noname

A quiet terminal front end for [Claude Code](https://claude.com/claude-code).
Claude Code runs underneath as the engine, so your subscription, plugins, skills, MCP servers and hooks all just work. noname only replaces what you look at.

Built for kitty with a transparent, blurred background: it never paints a background of its own.

## Install

```sh
cargo install --path .
```

Needs a logged-in `claude` on your `PATH`.

## Use

```sh
noname                  # new session in the current folder
noname -c               # continue the latest session here
noname -r <id>          # resume a specific session
noname --model sonnet   # any other claude flag is passed through
```

| key | |
|---|---|
| `enter` / `shift+enter` | send / new line |
| `esc` | stop claude, clear input |
| `esc esc` | rewind: code, conversation or both |
| `shift+tab` | mode: `›` default `?` plan `»` auto `!` bypass |
| `/` | command list, `↑↓` pick, `tab` or `→` complete |
| `→` | accept the suggested next prompt |
| `ctrl+o` / `ctrl+t` | details / todos |
| `y` `n` `a` | allow, deny, always allow |

`/btw <question>` asks a side question without touching the main conversation.
`/resume` picks an older session of this folder.

## Backgrounds

Type a name on its own and press enter, or open the picker with `/background`.

`stars` `cava` `lavat` `pipes` `asciiquarium` `cbonsai` `cmatrix` `unimatrix` `neo-matrix` `tty-clock` `weathr` `rain` `globe`

External programs run in a hidden terminal and are drawn at about 12 % brightness behind the text.

<p align="center">
<img src="assets/start.png" width="420">
<img src="assets/commands.png" width="420">
</p>

## How it works

noname starts `claude -p` in stream-json mode, renders its events with ratatui and answers permission requests over the control protocol. Closing the window kills every child process, including MCP servers.

`NONAME_MOCK=1 noname` runs a scripted fake session without calling Claude.
