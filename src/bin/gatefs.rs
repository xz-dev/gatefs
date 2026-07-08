fn main() {
    match gatefs::cli::main_entry() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("gatefs: {error}");
            std::process::exit(1);
        }
    }
}
