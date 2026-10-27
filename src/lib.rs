use anyhow::{Context, Result, bail};
use cli_command_kit::{
    HelpOptions, Mode,
    clap::{Arg, ArgAction, Command},
};
use std::{ffi::OsString, io::Write};

#[cfg(target_os = "linux")]
pub mod process;
#[cfg(target_os = "linux")]
pub mod socket_diag;
#[cfg(target_os = "linux")]
pub mod ui;

const SKILL: &str = "Install the porter skill using the Skills CLI:\n\n  npx skills add alexgorbatchev/porter --skill porter\n\nSkill:\n\n  https://github.com/alexgorbatchev/porter/tree/main/skills/porter\n";
pub fn command() -> Command {
    Command::new("porter")
        .about("Inspect listening TCP ports for this directory and its descendants")
        .arg(
            Arg::new("version")
                .long("version")
                .short('V')
                .action(ArgAction::SetTrue)
                .exclusive(true)
                .help("Print the raw version"),
        )
}
pub fn help_options() -> HelpOptions {
    HelpOptions {
        skill: Some(SKILL.into()),
        ..HelpOptions::default()
    }
}

fn perform(
    matches: &cli_command_kit::clap::ArgMatches,
    options: &HelpOptions,
    terminal: bool,
    output: &mut impl Write,
    mode: Mode,
) -> Result<()> {
    if cli_command_kit::write_skill(matches, options, output)? {
        return Ok(());
    }
    if matches.get_flag("version") {
        writeln!(output, "{}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    if !terminal {
        bail!("porter requires an interactive terminal on stdin and stdout");
    }
    #[cfg(target_os = "linux")]
    {
        ui::run(
            &std::env::current_dir().context("read current directory")?,
            mode,
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = mode;
        bail!("porter supports Linux only: socket diagnostics and /proc are required");
    }
}

pub fn entry(
    args: impl IntoIterator<Item = OsString>,
    terminal: bool,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    mode: Mode,
) -> u8 {
    let options = help_options();
    let root =
        cli_command_kit::setup(command(), mode, &options).expect("valid static command tree");
    let result = match root.try_get_matches_from(args) {
        Ok(matches) => perform(&matches, &options, terminal, output, mode),
        Err(error) if !error.use_stderr() => output
            .write_all(error.to_string().as_bytes())
            .map_err(Into::into),
        Err(error) => {
            let _ = write!(diagnostics, "{error}");
            return 1;
        }
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(
                diagnostics,
                "{} {error:#}",
                if mode == Mode::Agent {
                    "ERR:"
                } else {
                    "[ERROR]"
                }
            );
            1
        }
    }
}
