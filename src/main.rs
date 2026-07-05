#[tokio::main]
async fn main() {
    if let Err(error) = wat::run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
