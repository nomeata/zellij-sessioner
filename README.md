# session-manager-plus

A [Zellij](https://zellij.dev) plugin that enhances the built-in session manager
by showing **pane titles** for each session. See at a glance what's running
everywhere — particularly useful with tools like [Claude Code](https://claude.ai/code)
that put their status in the terminal title.

![screenshot](session-manager-plus.png)

Uses Zellij's [built-in UI components](https://zellij.dev/documentation/plugin-ui-rendering.html),
so it respects your theme.

## Install

### Pre-built wasm (recommended)

Use the plugin directly from a GitHub release URL — Zellij downloads and
caches it automatically:

```
zellij action launch-or-focus-plugin \
  https://github.com/joachimschmidt557/session-manager-plus/releases/latest/download/session-manager-plus.wasm \
  --floating
```

Or pin a specific version:

```
https://github.com/joachimschmidt557/session-manager-plus/releases/download/v0.1.0/session-manager-plus.wasm
```

### Keybinding

Add to your Zellij config (`~/.config/zellij/config.kdl`):

```kdl
shared_except "locked" {
    bind "Ctrl y" {
        LaunchOrFocusPlugin "https://github.com/joachimschmidt557/session-manager-plus/releases/latest/download/session-manager-plus.wasm" {
            floating true
        }
    }
}
```

### Build from source

Requires Rust with the `wasm32-wasip1` target.

```bash
# with Nix
nix develop && cargo build --release

# without Nix
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1
```

Then use the local build:

```bash
zellij action launch-or-focus-plugin \
  file:target/wasm32-wasip1/release/session-manager-plus.wasm \
  --floating
```

## Keybindings

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `Enter` | Switch to selected session |
| `d` | Delete selected dead session |
| `D` | Delete all dead sessions |
| `Esc` / `q` | Close the plugin |

## How it works

Subscribes to Zellij's `SessionUpdate` event which provides the full session
list with pane manifests. For each session it shows:

- **Session name** with status (attached / connected / exited)
- **Pane titles** indented underneath (plugin panes excluded)

Dead (resurrectable) sessions appear at the bottom with their age.
