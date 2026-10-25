//! `tokenscope` binary entry point.

fn main() {
    let code = tokenscope_cli::run(
        std::env::args_os(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    std::process::exit(code);
}
