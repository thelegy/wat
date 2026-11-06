fn main() {
    wat::complete();

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to initialize runtime: {error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = runtime.block_on(wat::run()) {
        if !wat::failure_reported(&error) {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}
