use anyhow::Result;
use log::Level;
use serde_json::Value;

use crate::event::{BackendEvent, Event, EventSender, LixEvent, LixField, LogEvent};

pub struct LixEventAdapter {
    events: EventSender,
}

impl LixEventAdapter {
    pub fn new(events: EventSender) -> Self {
        Self { events }
    }

    pub async fn handle_stderr_line(&mut self, line: &str) -> Result<()> {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            return Ok(());
        }

        if let Some(json_line) = line.strip_prefix("@nix ") {
            match serde_json::from_str::<Value>(json_line) {
                Ok(value) => self.handle_json(value).await,
                Err(error) => {
                    self.log(Level::Warn, &format!("invalid nix log JSON: {error}"))
                        .await?
                }
            }
            return Ok(());
        }

        self.log(Level::Info, line).await
    }

    pub async fn log_warning(&mut self, message: &str) -> Result<()> {
        self.log(Level::Warn, message).await
    }

    pub fn finish(&mut self, _success: bool) {}

    async fn handle_json(&mut self, value: Value) {
        let Some(action) = value.get("action").and_then(Value::as_str) else {
            return;
        };

        match action {
            "msg" => {
                let level = value.get("level").and_then(Value::as_u64).unwrap_or(3);
                if let Some(message) = value.get("msg").and_then(Value::as_str) {
                    let level = self.level_for_verbosity(level);
                    let level = if level == Level::Error
                        && message.starts_with("trace: evaluation warning:")
                    {
                        Level::Warn
                    } else {
                        level
                    };
                    let _ = self.log(level, message).await;
                }
            }
            "start" => {
                if let Some(event) = self.parse_start(&value) {
                    let _ = self
                        .events
                        .send(Event::Backend(BackendEvent::Lix(event)))
                        .await;
                }
            }
            "stop" => {
                if let Some(id) = value.get("id").and_then(Value::as_u64) {
                    let _ = self
                        .events
                        .send(Event::Backend(BackendEvent::Lix(LixEvent::Stop { id })))
                        .await;
                }
            }
            "result" => {
                if let Some(event) = self.parse_result(&value) {
                    let _ = self
                        .events
                        .send(Event::Backend(BackendEvent::Lix(event)))
                        .await;
                }
            }
            _ => {}
        }
    }

    fn parse_start(&self, value: &Value) -> Option<LixEvent> {
        let id = value.get("id").and_then(Value::as_u64)?;
        let kind = value.get("type").and_then(Value::as_u64).unwrap_or(0);
        let level = value.get("level").and_then(Value::as_u64).unwrap_or(3);
        let parent = value.get("parent").and_then(Value::as_u64).unwrap_or(0);
        let text = value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        Some(LixEvent::Start {
            id,
            kind,
            parent,
            level,
            text,
            fields: parse_fields(value.get("fields")),
        })
    }

    fn parse_result(&self, value: &Value) -> Option<LixEvent> {
        let id = value.get("id").and_then(Value::as_u64)?;
        let result_type = value.get("type").and_then(Value::as_u64)?;

        Some(LixEvent::Result {
            id,
            result_type,
            fields: parse_fields(value.get("fields")),
        })
    }

    async fn log(&mut self, level: Level, message: &str) -> Result<()> {
        // Concurrent evaluations can contend on Nix's optional evaluation cache.
        // Nix explicitly ignores this failure; retain it only in the raw recording.
        if message.starts_with("error (ignored): ")
            && message.contains("SQLite database '")
            && message.contains("/eval-cache-")
            && message.contains(".sqlite")
            && message.contains("' is busy")
        {
            return Ok(());
        }

        // Nix retries a busy store database; its nested diagnostic prefixes
        // make this temporary contention look like a failed build.
        let plain = crate::text::without_ansi(message);
        let mut diagnostic = plain.as_ref();
        while let Some(rest) = diagnostic
            .strip_prefix("warning: ")
            .or_else(|| diagnostic.strip_prefix("error: "))
            .or_else(|| diagnostic.strip_prefix("error (ignored): "))
        {
            diagnostic = rest;
        }
        let (level, message) = if diagnostic.starts_with("SQLite database '")
            && diagnostic.ends_with("' is busy")
        {
            (Level::Info, diagnostic)
        } else {
            (level, message)
        };

        self.events
            .send(Event::Log(LogEvent {
                level,
                message: message.to_string(),
            }))
            .await
    }

    fn level_for_verbosity(&self, level: u64) -> Level {
        match level {
            0 => Level::Error,
            1 => Level::Warn,
            2..=4 => Level::Info,
            5..=6 => Level::Debug,
            _ => Level::Trace,
        }
    }
}

fn parse_fields(value: Option<&Value>) -> Vec<LixField> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| match item {
            Value::Number(number) => number.as_u64().map(LixField::Int),
            Value::String(value) => Some(LixField::Str(value.clone())),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventBus;

    #[tokio::test]
    async fn reports_database_contention_as_info_without_diagnostic_prefixes() {
        let (bus, mut stream) = EventBus::new(16);
        let mut adapter = LixEventAdapter::new(bus.sender());
        let diagnostic = "SQLite database '/nix/var/nix/db/db.sqlite' is busy";
        for prefix in ["", "error: ", "warning: error: ", "error (ignored): error: "] {
            let message = format!("{prefix}{diagnostic}");
            adapter.handle_stderr_line(&message).await.unwrap();
            for level in [0, 1] {
                adapter.handle_stderr_line(&format!("@nix {}", serde_json::json!({
                    "action": "msg", "level": level, "msg": message
                }))).await.unwrap();
            }
        }
        let colored = "\x1b[35;1mwarning:\x1b[0m \x1b[31;1merror:\x1b[0m SQLite database '\x1b[35;1m/nix/var/nix/db/db.sqlite\x1b[0m' is busy";
        adapter.handle_stderr_line(colored).await.unwrap();
        adapter.handle_stderr_line(&format!("@nix {}", serde_json::json!({
            "action": "msg", "level": 0, "msg": colored
        }))).await.unwrap();
        drop(adapter);
        drop(bus);
        let mut count = 0;
        while let Some(envelope) = stream.recv().await {
            let Event::Log(log) = envelope.event else { panic!("expected log") };
            assert_eq!(log.level, Level::Info);
            assert_eq!(log.message, diagnostic);
            count += 1;
        }
        assert_eq!(count, 14);
    }

    #[tokio::test]
    async fn hides_ignored_eval_cache_contention_but_preserves_other_errors() {
        let (bus, mut stream) = EventBus::new(16);
        let mut adapter = LixEventAdapter::new(bus.sender());
        let ignored = "error (ignored): \x1b[35;1m\x1b[31;1merror:\x1b[0m SQLite database '\x1b[35;1m/home/user/.cache/nix/eval-cache-v5/hash.sqlite\x1b[0m' is busy\x1b[0m";
        for message in [ignored, "error (ignored): error: SQLite database '/cache/eval-cache-v5/hash.sqlite' is busy"] {
            adapter.handle_stderr_line(message).await.unwrap();
            adapter.handle_stderr_line(&format!("@nix {}", serde_json::json!({
                "action": "msg", "level": 0, "msg": message
            }))).await.unwrap();
        }
        let visible = [
            "error (ignored): error: SQLite database '/cache/eval-cache-v5/hash.sqlite' is corrupt",
            "another diagnostic",
        ];
        for message in visible {
            adapter.handle_stderr_line(&format!("@nix {}", serde_json::json!({
                "action": "msg", "level": 0, "msg": message
            }))).await.unwrap();
        }
        drop(adapter);
        drop(bus);
        for expected in visible {
            let envelope = stream.recv().await.unwrap();
            let Event::Log(log) = envelope.event else { panic!("expected log") };
            assert_eq!(log.message, expected);
        }
        assert!(stream.recv().await.is_none());
    }
}
