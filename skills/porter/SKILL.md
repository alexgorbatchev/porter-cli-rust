---
name: porter
description: Inspect and terminate Linux listening TCP processes whose current working directory is the current directory or a descendant.
metadata:
  created_on: 2026-10-14 15:12
  last_modified: 2026-10-14 15:12
  status: current
---

# Usage

Run `porter` from the directory to inspect. Both stdin and stdout must be interactive terminals. Linux 5.3+ with socket diagnostics, `/proc`, and pidfd support is required. Listening IPv4 and IPv6 TCP sockets are correlated with process file descriptors; rows match the process's current working directory, including descendants. Current working directory is not a durable launch-origin record. Rows sort by PID, port, and displayed command. Duplicate descriptors for one port yield one row per process and port.

Use `j` or Down to select the next row, `k` or Up for the previous row, `r` to refresh, and `q` or Ctrl-C to quit. `x` sends SIGTERM to the selected process through its retained kernel handle. This affects all sockets belonging to that process and does not use SIGKILL. An exited process's handle cannot signal a new process that reuses its PID. Rows refresh every 750 milliseconds and 200 milliseconds after `x`. Errors and signal results appear in the status line. Screens adapt to the actual terminal dimensions.

Commands inside this directory appear relative to it; external executable paths show their basename. Paths under the user's home are displayed using `~`. Empty arguments and terminal control characters are quoted. Metadata inaccessible because of permissions or process exit is skipped. Socket diagnostics failures appear in the status line and retain the last successful rows; `r` retries discovery.

`porter --version` or `porter -V` prints the raw package version and newline; `--version` has type bool, defaults to false, and is exclusive with other options. `porter --help` or `porter -h` prints help; `--help` has type bool and defaults to false.

`porter help` prints root help, `porter help skill` prints skill-command help, and `porter help help` prints generated-help help. `porter skill --help` prints skill-command help. There are no completion commands.

`porter skill` accepts no positional arguments or command-specific flags. It prints the Skills CLI install command and canonical skill URL offline. The URL addresses the original repository and does not establish publication of this Rust migration.

`AGENT=1`, `AGENT=true`, or `AGENT=yes` selects compact formatting. Every agent help screen begins with the alert to read `AGENT=1 porter skill`. The interactive display marks the selected row with `* ` in agent mode and `> ` in human mode. Metadata and raw version output have identical bytes in both modes. Runtime errors are printed once to stderr with `ERR:` in agent mode or `[ERROR]` in human mode. Success exits 0; arguments, unsupported runtime, terminal, rendering, event, or restoration failures exit 1. Runtime failures print no usage screen.

The tool reads kernel metadata and uses a raw alternate terminal screen during interaction, restoring the terminal on normal exit and returned errors. It writes no persistent files, makes no network requests to external services, and sends SIGTERM only when `x` is pressed on a selected row.

```sh
cd project
porter
AGENT=1 porter --help
porter --version
```
