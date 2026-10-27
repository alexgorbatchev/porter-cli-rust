use anyhow::{Context, Result};
use procfs::{
    ProcError,
    process::{FDTarget, Process},
};
use rustix::{
    fd::OwnedFd,
    process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal},
};
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone)]
pub struct Entry {
    pub pid: i32,
    pub port: u16,
    pub command: String,
    handle: Arc<OwnedFd>,
}

impl Entry {
    pub fn terminate(&self) -> Result<()> {
        pidfd_send_signal(&self.handle, Signal::TERM)
            .with_context(|| format!("send SIGTERM to PID {}", self.pid))
    }
}

pub fn within_root(root: &Path, candidate: &Path) -> bool {
    !root.as_os_str().is_empty()
        && !candidate
            .components()
            .any(|part| part == Component::ParentDir)
        && candidate.starts_with(root)
}

fn quote(value: &str) -> String {
    if value.is_empty()
        || value
            .chars()
            .any(|value| value.is_whitespace() || value.is_control() || value == '"')
    {
        serde_json::to_string(value).expect("string serialization is infallible")
    } else {
        value.into()
    }
}

fn display_part(root: &Path, text: &str, executable: bool) -> String {
    let path = Path::new(text);
    let text = if within_root(root, path) {
        path.strip_prefix(root)
            .expect("verified prefix")
            .to_string_lossy()
            .into_owned()
    } else if executable && path.is_absolute() {
        path.file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned()
    } else {
        text.into()
    };
    quote(&cli_command_kit::display_path(Path::new(
        if text.is_empty() { "." } else { &text },
    )))
}

pub fn display_command(root: &Path, cwd: &Path, bytes: &[u8], executable: Option<&Path>) -> String {
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    let parts = if bytes.is_empty() {
        Vec::new()
    } else {
        bytes
            .split(|byte| *byte == 0)
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect::<Vec<_>>()
    };
    let command = if parts.is_empty() {
        executable
            .map(|path| display_part(root, &path.to_string_lossy(), true))
            .unwrap_or_else(|| "<unknown>".into())
    } else {
        parts
            .iter()
            .enumerate()
            .map(|(index, part)| {
                if part.is_empty() {
                    "\"\"".into()
                } else {
                    display_part(root, part, index == 0)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let relative = cwd.strip_prefix(root).unwrap_or(cwd);
    if relative.as_os_str().is_empty() {
        command
    } else {
        format!(
            "{} $ {command}",
            quote(&cli_command_kit::display_path(relative))
        )
    }
}

fn ignorable(error: &ProcError) -> bool {
    matches!(
        error,
        ProcError::NotFound(_) | ProcError::PermissionDenied(_) | ProcError::Incomplete(_)
    )
}

pub struct Scanner {
    root: PathBuf,
}
impl Scanner {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn scan(&self) -> Result<Vec<Entry>> {
        let ports = crate::socket_diag::listeners()?;
        let mut rows = Vec::new();
        for directory in std::fs::read_dir("/proc").context("read process directory")? {
            let directory = directory?;
            let Some(pid) = directory
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<i32>().ok())
                .and_then(Pid::from_raw)
            else {
                continue;
            };
            // Acquire the kernel handle before reading metadata. PID reuse can
            // make this handle stale, but can never redirect it to a new task.
            let handle = match pidfd_open(pid, PidfdFlags::empty()) {
                Ok(handle) => Arc::new(handle),
                Err(
                    rustix::io::Errno::SRCH | rustix::io::Errno::ACCESS | rustix::io::Errno::PERM,
                ) => continue,
                Err(error) => {
                    return Err(error).context("open process handle; Linux 5.3+ is required");
                }
            };
            match self.process(pid.as_raw_nonzero().get(), handle, &ports) {
                Ok(mut entries) => rows.append(&mut entries),
                Err(error) if error.downcast_ref::<ProcError>().is_some_and(ignorable) => continue,
                Err(error) => return Err(error),
            }
        }
        rows.sort_by(|left, right| {
            (left.pid, left.port, &left.command).cmp(&(right.pid, right.port, &right.command))
        });
        Ok(rows)
    }

    fn process(
        &self,
        pid: i32,
        handle: Arc<OwnedFd>,
        listeners: &crate::socket_diag::Listeners,
    ) -> Result<Vec<Entry>> {
        let process = Process::new(pid)?;
        let cwd = process.cwd()?;
        if !within_root(&self.root, &cwd) {
            return Ok(Vec::new());
        }
        let mut ports = BTreeSet::new();
        for fd in process.fd()? {
            let fd = match fd {
                Ok(fd) => fd,
                Err(error) if ignorable(&error) => continue,
                Err(error) => return Err(error.into()),
            };
            if let FDTarget::Socket(inode) = fd.target
                && let Some(found) = listeners.get(&inode)
            {
                ports.extend(found);
            }
        }
        if ports.is_empty() {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::new();
        process
            .open_relative("cmdline")?
            .read_to_end(&mut bytes)
            .context("read process command")?;
        let executable = if bytes.is_empty() {
            process.exe().ok()
        } else {
            None
        };
        let command = display_command(&self.root, &cwd, &bytes, executable.as_deref());
        Ok(ports
            .into_iter()
            .map(|port| Entry {
                pid,
                port,
                command: command.clone(),
                handle: Arc::clone(&handle),
            })
            .collect())
    }
}
