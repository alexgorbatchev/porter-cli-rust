use std::io::IsTerminal;
fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(porter_cli::entry(
        std::env::args_os(),
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
        cli_command_kit::Mode::from_env(),
    ))
}
