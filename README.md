# 🐺 awo 🦊

**awo** is a lightweight process manager written in Rust. It was born from a wild desire for something simpler than the existing options. awo manages your "pack" of processes via a TOML file, providing a comfortable environment for developing and running multiple services simultaneously.

⚠️ *Project is still in development; some bugs may be lurking in the woods. PRs and issues are welcome!*

# 🐾 Main Features

- **Event-driven architecture** 🌙 based on `tokio` for asynchronous process running and handling.
- **Output Tracking** 🐕 Getting stdout/stderr output from your processes.
- **Flexible Instincts** ⚙️ Define exactly how awo behaves on process exit or error: restart the process, shut down the whole pack, or simply ignore it.
- **Silent Mode** 🤫 Optional hiding of process output to keep your terminal clean.

## 🌲 Future Evolution (TODOs)

- [ ] Scheduling periodic processes ⏰
- [ ] Optional Web UI 🌐
- [ ] Use `notify` crate for filesystem watching instead of manual implementation 👁️
- [ ] Defining signals for restart/shutdown ⚡
- [ ] Granular output control (Hide stdout/stderr separately) 🤐

# 🗺️ Territory Restrictions

awo **does not support Windows** at this time 🚫🪟 due to the usage of the [nix crate](https://docs.rs/nix/latest/nix/).

# 🛠️ Joining the Pack

## Statically linked binaries
Grab the pre-compiled binaries from the latest [release of awo](https://github.com/mrtstg/awo/releases/latest).

## Build from source
```bash
git clone https://github.com/mrtstg/awo
cd awo
cargo install --path .
# or, build statically linked binaries using glibc or musl (make required)
make build-static-glibc
make build-static-musl # require musl-tools
```

# 🚀 Usage
```bash
awo [FLAGS]
```

| Flag | Description |
|------|-------------|
| `-n` | Disable coloring of process names and environment variables (`FORCE_COLOR`, `CLICOLOR_FORCE`, `COLORTERM`) |
| `-s` | Prints a sample config to stdout 📋 |
| `-h` | Show help page 📖 |
| `-V` | Print the version 🏷️ |
| `-c <path>` | Use a custom path for the config file [default: `./awo.toml`] |
| `-e <key>`/`--except <key>` | Run config, excluding app with provided key. For example `-e example` for config below. Can be used multiple times. |

The manager can be gracefully stopped by `<Ctrl+C>` or by sending `SIGINT`.

# 📋 Configuration Reference
```toml
align = true
on_exit = "exit"
on_error = "exit"
restart_delay = 1

[run.example]
command = "echo 'Hello world!'"
name = "optional_custom_name"
on_exit = "ignore"
on_error = "restart"
restart_delay = 5
watch = ["/tmp/somefile"]
color = 7
hide = false
cwd = "/usr/bin/"

[run.example.env]
ENV1 = "VALUE1"
ENV2 = "VALUE2"
```

awo looks for an `awo.toml` file in the working directory by default. You can generate a sample config by running `awo --sample`.


## ⚙️ Global Settings
| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `align` | Boolean | true | Pads process names in the logs for better vertical alignment. |
| `on_exit` | String | "ignore" | Action to take when a process exits with code 0. |
| `on_error` | String | "exit" | Action to take when a process exits with a non-zero code. |
| `restart_delay` | Integer | 1 | Seconds to wait before restarting a process. |
| `run` | Table | N/A | A map of process definitions to execute. |


## 🐕 Processes
Each process is defined in the `[run.<unique_name>]` section.


| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `command` | String | Required | The shell command to execute. (Pipes `&&`, `|` not recommended). |
| `name` | String | Map Key | Custom name used in logs. |
| `on_exit` | String | Global | Overrides global `on_exit` behavior. |
| `on_error` | String | Global | Overrides global `on_error` behavior. |
| `restart_delay` | Integer | Global | Overrides global `restart_delay` (in seconds). |
| `watch` | Array | [] | Paths or glob patterns. Process restarts on change. |
| `color` | Integer | Random | ANSI color code (1-255, see [color-chart](https://upload.wikimedia.org/wikipedia/commons/1/15/Xterm_256color_chart.svg)). |
| `hide` | Boolean | false | If true, process logs won't be printed. |
| `cwd` | String | Current dir | Changes working directory of the process. |
| `env` | Map | Empty | Environment variables of process |


## 🔄 Process Behavior
Used in `on_exit` and `on_error` fields.

| Value | Effect |
|-------|--------|
| `exit` | Shuts down the process manager and the entire pack. |
| `restart` | Restarts the process after the specified delay. |
| `ignore` | Does nothing; the process remains stopped. |


# 📜 License
Project is licensed under the BSD-3-Clause license.
