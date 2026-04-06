fn main() {
    if let Err(error) = wat::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
