use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    sandbox: String,
}

fn main() {
    let args = Args::parse();
    match gatefs::tui::run(args.sandbox) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("gatefs-access-tui: {error}");
            std::process::exit(1);
        }
    }
}
