mod backend;
pub mod cli;
mod config;
mod event;
mod operations;
mod palette;
mod planner;
mod recording;
mod reducer;
mod reporter;
mod results;
mod text;

use std::{io::Write, path::Path, sync::Once};

use anyhow::{Result, bail};
use clap::Parser;

static INIT_LOGGER: Once = Once::new();

pub async fn run() -> Result<()> {
    init_logging();
    let cli = cli::Cli::parse();
    if let cli::Command::Replay { path } = &cli.command {
        return recording::replay(path).await;
    }
    let report_path = match &cli.command {
        cli::Command::Build(args) => args.results_file.clone(),
        _ => None,
    };
    let report = results::Report::default();
    if let Some(path) = &report_path {
        report.write(path)?;
    }
    let result = async {
        prepare_environment()?;
        recording::init()?;
        operations::execute(cli.command, report.clone()).await
    }.await;
    if let Some(path) = &report_path {
        report.finish(&result);
        report.write(path)?;
    }
    recording::write(
        serde_json::json!({"kind": "outcome", "error": result.as_ref().err().map(|error| format!("{error}"))}),
    );
    result
}

pub fn complete() {
    use clap::CommandFactory;
    use clap_complete::CompleteEnv;

    CompleteEnv::with_factory(cli::Cli::command).complete();
}

thread_local! {
    static LOG_TIME: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

fn with_log_time(time: Option<&str>, action: impl FnOnce()) {
    struct Reset(Option<String>);
    impl Drop for Reset {
        fn drop(&mut self) {
            LOG_TIME.with(|time| *time.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(LOG_TIME.with(|current| current.replace(time.map(str::to_owned))));
    action();
}

fn init_logging() {
    INIT_LOGGER.call_once(|| {
        let env = env_logger::Env::default().default_filter_or("info");
        if let Err(err) = env_logger::Builder::from_env(env)
            .format(|buffer, record| {
                let style = palette::log_level_style(buffer, record.level());
                let now = jiff::Zoned::now();
                let time = LOG_TIME.with(|time| time.borrow().clone())
                    .unwrap_or_else(|| now.strftime("%H:%M:%S").to_string());
                writeln!(
                    buffer,
                    "[{} {} {style}{:>5}{style:#}] {}",
                    time,
                    record.target(),
                    record.level(),
                    record.args()
                )
            })
            .try_init()
        {
            eprintln!("failed to initialize logger: {err}");
        }
    });
}

fn prepare_environment() -> Result<()> {
    if !Path::new("flake.nix").exists() {
        log::error!("Error: ./flake.nix not found in current directory");
        bail!("flake.nix missing");
    }

    Ok(())
}

pub fn failure_reported(error: &anyhow::Error) -> bool {
    operations::failure_reported(error)
}
