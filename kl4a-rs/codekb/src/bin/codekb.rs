use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = env::args().skip(1).collect();
    let code = codekb::cli::main(&argv);
    ExitCode::from(code as u8)
}
