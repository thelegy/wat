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
