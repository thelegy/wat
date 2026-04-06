use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "wat")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Todo,
}

fn main() {
    let args = Cli::parse();
    println!("Hello, world!");
    println!("Args: {args:?}");
}
