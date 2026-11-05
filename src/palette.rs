use std::io::IsTerminal;

use crate::event::PipelinePhase;

pub const FAILED: u8 = 1;
pub const DONE: u8 = 2;
pub const SPINNER_STYLE: &str = "yellow";
pub const MUTED_STYLE: &str = "dim";

pub fn regular(text: &str, color: u8) -> String {
    format!("\x1b[38;5;{color}m{text}\x1b[39m")
}

pub fn muted(text: &str) -> String {
    format!("\x1b[2m{text}\x1b[0m")
}

pub fn muted_stage(text: &str, phase: PipelinePhase) -> String {
    muted(&regular(text, stage_color(phase)))
}

pub fn log_level_style(
    buffer: &env_logger::fmt::Formatter,
    level: log::Level,
) -> env_logger::fmt::style::Style {
    buffer.default_level_style(level)
}

pub fn stage_color(phase: PipelinePhase) -> u8 {
    match phase {
        PipelinePhase::Evaluate => 68, // blue
        PipelinePhase::Build => 75,    // sky blue
        PipelinePhase::Upload => 220,  // golden yellow
        PipelinePhase::Deploy | PipelinePhase::Activate | PipelinePhase::Reboot => 214, // amber orange
    }
}

pub fn colors_enabled() -> bool {
    std::io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").as_deref() != Ok("dumb")
}
