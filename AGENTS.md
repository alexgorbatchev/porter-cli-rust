---
created_on: 2026-10-14 15:17
last_modified: 2026-10-14 15:17
status: current
---

# porter

Linux interactive TCP listener inspector. Inherit [../AGENTS.md](../AGENTS.md).

## Commands

- Build: `just build` writes `bin/porter`.
- Run: `just run`; agent mode: `just run-ai`.
- Test: `just test`; strict lint: `just lint`; full gate: `just check`.
- Coverage: `just coverage`, at least 90% lines.
- Native interactive inspection uses the termctrl skill and project `.tmp/` evidence.

## Contracts

- Keep Linux-only listener discovery on native NETLINK_SOCK_DIAG with IPv4 and IPv6 TCP_LISTEN requests. Use maintained packet codecs; reject malformed, failed, overrun, and interrupted dumps instead of presenting a partial result as complete.
- Use procfs native process directory handles for cwd, descriptors, and command bytes. Preserve empty NUL-delimited arguments and quote control characters before display. Match descendant path components, including legitimate names beginning `..`.
- Acquire pidfd handles before reading process metadata; keep them with the displayed rows. Send only SIGTERM through pidfd_send_signal. Never replace process handles with numeric kill calls that can target a reused PID.
- Preserve the original 750ms refresh and 200ms signal follow-up. Use native crossterm polling and event decoding; keep the loop free of detached blocking input readers.
- Render with native Ratatui tables, actual terminal dimensions, and a visible status line. Return render/event/restoration failures, restoring the terminal before reporting at the single `entry` boundary.
- Use cli-command-kit for clap help, skill pointers, agent output, and home folding. Update `skills/porter/SKILL.md` and its `last_modified` with any command, argument, flag, default, environment, output, or side-effect change; verify against live command metadata.
- Runtime-result changes require a behavioral test-file change and at least 90% line coverage; scripts are excluded. Prove regressions by disabling behavior, restore it, and rerun the gate.
- Signal tests must create their own disposable listener processes and operate only on their retained handles. Keep temporary files in `.tmp/` and scope formatters to this package.
- Never publish releases, tags, packages, or deployments without explicit authorization. Existing Go release URLs do not distribute this Rust project.
