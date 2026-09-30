# Pomme Server

Pomme Server is the headless server application in the Pomme workspace. It
accepts TCP clients, has an optional terminal status renderer, and runs plugins
as isolated child processes. The current wire protocol is an administration
and health-check protocol; Minecraft gameplay packet handling is the next
server layer and is not claimed by this initial server foundation.

## Run

```bash
cargo run -p pomme-server -- --host 0.0.0.0 --port 25565
```

For Docker, systemd, and other headless environments, disable all terminal
drawing:

```bash
cargo run -p pomme-server -- --rendering off
```

Use `--help` to see all options. Set `RUST_LOG`, for example
`RUST_LOG=pomme_server=debug`, to change logging verbosity. Stop cleanly with
Ctrl+C.

## Protocol

On connection the server writes `POMME 1 READY`. Commands are UTF-8 lines:

| Command | Reply |
| --- | --- |
| `PING` | `PONG` |
| `QUIT` | `BYE`, then disconnect |

Unknown commands receive `ERROR unknown command`.

## Plugins

Put manifests ending in `.plugin.json` in `plugins/` (or select another folder
with `--plugins-dir`). Example:

```json
{
  "name": "welcome-log",
  "executable": "welcome-log",
  "enabled": true
}
```

Relative executable paths are resolved from the plugin directory. Every
enabled plugin is invoked once for each lifecycle event and receives one JSON
object on standard input. Supported event names are:

- `server_started` (`address`)
- `client_connected` (`peer`)
- `client_disconnected` (`peer`)
- `server_stopping`

Plugins may write normal output to the server's stdout/stderr. A plugin crash
or non-zero exit is logged and does not take down the server. Plugin processes
run with the server account's permissions: only install trusted plugins.

Minimal POSIX shell plugin:

```sh
#!/bin/sh
read event
printf 'plugin received: %s\n' "$event"
```

Make it executable with `chmod +x plugins/welcome-log`.

## Test

```bash
cargo test -p pomme-server
cargo clippy -p pomme-server --all-targets -- -D warnings
```
