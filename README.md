`porter` shows listening TCP ports for Linux processes working in the current directory or its descendants. It helps developers inspect local servers and stop a selected process from an interactive terminal.

# What It Does

- **Inspect listeners:** Show process IDs, TCP ports, and commands for matching working directories.
- **Follow changes:** Refresh automatically and keep the selected process visible.
- **Stop a process:** Send SIGTERM with `x` to the selected row.

# How It Works

- Change into the directory you want to inspect and run `porter`.
- Move between rows with `j`/`k` or the arrow keys.
- Press `r` to refresh, `x` to send SIGTERM, or `q`/Ctrl-C to quit.

# How it Really Works

- Linux socket diagnostics identify IPv4 and IPv6 TCP listeners; process file descriptors identify their owners. Permissions can hide some processes.
- Matching uses each process's current working directory. It does not identify where a process was originally launched.
- Rows refresh every 750 milliseconds and 200 milliseconds after a signal. Errors and signal results appear in the status line.
- SIGTERM targets a retained kernel process handle. A process that reuses an exited process's ID cannot receive a signal through its old handle. The signal affects the process, including its other ports.
- Commands within this directory appear as relative paths; external executables appear by basename. Home paths use `~`; empty and control-containing arguments are quoted.
- The interface uses the actual terminal size and restores the terminal when quitting or returning an error. Both stdin and stdout must be terminals.
- `AGENT=1` selects compact help and error formatting. Success returns exit 0; argument, terminal, rendering, event, and restoration failures return exit 1. The tool writes no persistent files or external network requests.

# Prerequisites

[Linux 5.3 or newer](https://man7.org/linux/man-pages/man2/pidfd_open.2.html), with socket diagnostics, `/proc`, and process-handle support. An interactive terminal is required.

# Installation

Rust release binaries are unpublished. Contributor commands are in [AGENTS.md](AGENTS.md). The canonical [source repository](https://github.com/alexgorbatchev/porter) and skill pointer do not establish publication of this Rust implementation.

# Quick Start

```sh
cd project
porter
porter --help
```

Sample screen for a project with one local server:

```text
cwd ~/project
  PID     PORT   COMMAND
> 1234    3000   server .
Tracking 1 listening process entries
j/k: select  x: SIGTERM  r: refresh  q: quit
```

# Options & Flags

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--help` | `-h` | `false` | Print help |
| `--version` | `-V` | `false` | Print the raw version |

`porter skill` prints the installation command and URL for the [operational reference](skills/porter/SKILL.md).

# License

[MIT License](LICENSE), copyright 2026 Alex Gorbatchev.
