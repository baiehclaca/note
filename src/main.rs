#![forbid(unsafe_code)]

fn main() {
    let code = match note::run(std::env::args_os().skip(1)) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            error.code()
        }
    };
    std::process::exit(code);
}
