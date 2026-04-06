pub struct Backend {
    binary: String,
}

impl Backend {
    pub fn detect() -> Self {
        let binary = std::env::var("WAT_NIX_BINARY").unwrap_or_else(|_| "nix".to_string());
        Self::new(binary)
    }

    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub fn binary(&self) -> &str {
        &self.binary
    }

    pub fn toplevel_attr(&self, hostname: &str) -> String {
        format!(".#nixosConfigurations.{hostname}.config.system.build.toplevel")
    }

    pub fn iso_attr(&self, hostname: &str) -> String {
        format!(".#nixosConfigurations.{hostname}.config.system.build.iso")
    }

    pub fn sdcard_attr(&self, hostname: &str) -> String {
        format!(".#nixosConfigurations.{hostname}.config.system.build.sdImage")
    }
}
