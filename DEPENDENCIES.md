---
created_on: 2026-10-14 15:17
last_modified: 2026-10-14 15:17
status: current
---

# Native dependency choices

| Go functionality | Rust dependency |
| :--- | :--- |
| Handwritten sock_diag structures and netlink parsing | netlink-packet-sock-diag 0.5.0, netlink-packet-core 0.9.0, netlink-sys 0.9.0 |
| `/proc` process metadata | procfs 0.18.0 native process directory handles, fd target enum, and open_relative for raw cmdline bytes |
| syscall.Kill with a numeric PID | rustix 1.1.5 native pidfd_open and pidfd_send_signal(SIGTERM) |
| x/term raw mode and handwritten input/rendering | Ratatui 0.30.2 and crossterm 0.29.0 native terminal lifecycle, events, tables, and clipping |
| Manual argv handling | cli-command-kit native clap command tree |
| Behavioral terminal tests | portable-pty 0.9.0 and vt100 0.16.2 native PTY and terminal screen decoding |

The direct dependencies use MIT, MIT/Apache-2.0, or compatible Rustix license options. The Go MIT notice is retained. APIs were checked in downloaded source and official [sock_diag](https://github.com/rust-netlink/netlink-packet-sock-diag), [Ratatui](https://ratatui.rs/tutorials/counter-app/basic-app/), and [pidfd_send_signal](https://docs.rs/rustix/1.1.5/rustix/process/fn.pidfd_send_signal.html) documentation.

Tests reuse the complete, patched portable-pty source owned by the sibling tui-frame project. Its [provenance and native descriptor fixes](../tui-frame/DEPENDENCIES.md) apply here too. Runtime binaries do not depend on portable-pty or vt100. No custom terminal emulator or socket protocol layout is implemented.

# Corrected source behavior

- The Go path check rejects legitimate descendants with names beginning `..`; Rust matches path components.
- Go command splitting drops empty arguments; Rust removes exactly the final NUL terminator and preserves empty argument positions.
- Go sends SIGTERM to a numeric PID selected in an earlier scan. Rust retains a kernel process handle per displayed process, preventing PID reuse from redirecting the signal. Linux 5.3+ is therefore required; unsupported kernels report the discovery failure visibly instead of falling back to unsafe PID signaling.
- Go calculates status text but omits it from the screen, ignores render write failures, and expands tiny viewports to at least 40×3. Rust shows status, propagates writes, and clips to the actual viewport through native widgets.
- Go starts a blocking input goroutine without shutdown coordination. Rust polls and reads events in the same loop, with no input worker.
- Go prints some returned errors before main prints them again. Rust reports each error at one boundary; raw version output follows the shared CLI contract.

Discovery follows current cwd semantics and skips disappearing or inaccessible process metadata. It does not discover listening sockets in other network namespaces. No external services, lsof, or ss are used. Native Linux x86_64 discovery, process signaling, rendering, resize, refresh, and terminal restoration are verified in [.tmp/VERIFICATION.md](.tmp/VERIFICATION.md). Other operating systems are unsupported.
