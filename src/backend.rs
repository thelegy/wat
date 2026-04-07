use std::path::Path;

use anyhow::Result;

mod lix;

use lix::LixBackend;

pub fn detect() -> Box<dyn Backend + Send + Sync> {
    let binary = std::env::var("WAT_NIX_BINARY").unwrap_or_else(|_| "nix".to_string());
    Box::new(LixBackend::new(binary))
}

pub trait Backend {
    fn build_installable(
        &self,
        installable: &str,
        out_link: &Path,
        keep_going: bool,
        log_format: LogFormat,
    ) -> Result<()>;

    fn copy_closure(
        &self,
        source: &Path,
        destination: &str,
        substitute_on_destination: bool,
        log_format: LogFormat,
    ) -> Result<()>;
    fn list_machines(&self) -> Result<Vec<String>>;
}

#[derive(Clone, Copy)]
pub enum LogFormat {
    BarWithLogs,
}

impl LogFormat {
    fn as_arg(self) -> &'static str {
        match self {
            LogFormat::BarWithLogs => "bar-with-logs",
        }
    }
}
