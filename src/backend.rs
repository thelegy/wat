use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::event::EventSender;

mod lix;
pub(crate) mod lix_parser;

use lix::LixBackend;

pub fn detect() -> Arc<dyn Backend + Send + Sync> {
    let binary = std::env::var("WAT_NIX_BINARY").unwrap_or_else(|_| "nix".to_string());
    Arc::new(LixBackend::new(binary))
}

#[async_trait]
pub trait Backend {
    async fn build_installable(
        &self,
        installable: &str,
        out_link: &Path,
        keep_going: bool,
        events: EventSender,
    ) -> Result<()>;

    async fn copy_closure(
        &self,
        source: &Path,
        destination: &str,
        substitute_on_destination: bool,
        events: EventSender,
    ) -> Result<()>;

    async fn list_machines(&self) -> Result<Vec<String>>;
}
