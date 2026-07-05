use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;

mod lix;
mod lix_renderer;

use lix::LixBackend;

pub fn detect() -> Box<dyn Backend + Send + Sync> {
    let binary = std::env::var("WAT_NIX_BINARY").unwrap_or_else(|_| "nix".to_string());
    Box::new(LixBackend::new(binary))
}

#[async_trait]
pub trait Backend {
    async fn build_installable(
        &self,
        installable: &str,
        out_link: &Path,
        keep_going: bool,
    ) -> Result<()>;

    async fn copy_closure(
        &self,
        source: &Path,
        destination: &str,
        substitute_on_destination: bool,
    ) -> Result<()>;

    async fn list_machines(&self) -> Result<Vec<String>>;
}
