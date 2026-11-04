use std::path::PathBuf;

use anyhow::{Context, Result};
use log::Level;
use tokio::sync::mpsc;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct EventScope {
    pub host: Option<String>,
    pub phase: Option<PipelinePhase>,
}

impl EventScope {
    fn merge(&self, overlay: EventScope) -> Self {
        Self {
            host: overlay.host.or_else(|| self.host.clone()),
            phase: overlay.phase.or(self.phase),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct EventEnvelope {
    pub scope: EventScope,
    pub event: Event,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum Event {
    Pipeline(PipelineEvent),
    Host(HostEvent),
    Backend(BackendEvent),
    Log(LogEvent),
    Artifact(ArtifactEvent),
    Output { stderr: bool, bytes: Vec<u8> },
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum PipelineEvent {
    Planned {
        stages: Vec<(String, Vec<PipelinePhase>)>,
    },
    Started {
        hosts: Vec<String>,
    },
    PhaseStarted(PipelinePhase),
    PhaseFinished {
        phase: PipelinePhase,
        success: bool,
    },
    Finished {
        success: bool,
    },
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelinePhase {
    Evaluate,
    Build,
    Deploy,
    Upload,
    Activate,
    Reboot,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum HostEvent {
    Started {
        host: String,
        phase: PipelinePhase,
    },
    Finished {
        host: String,
        phase: PipelinePhase,
        success: bool,
    },
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum ArtifactEvent {
    Ready { host: String, path: PathBuf },
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct LogEvent {
    pub level: Level,
    pub message: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum BackendEvent {
    Lix(LixEvent),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum LixEvent {
    Start {
        id: u64,
        kind: u64,
        parent: u64,
        level: u64,
        text: String,
        fields: Vec<LixField>,
    },
    Stop {
        id: u64,
    },
    Result {
        id: u64,
        result_type: u64,
        fields: Vec<LixField>,
    },
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum LixField {
    Int(u64),
    Str(String),
}

#[derive(Clone)]
pub struct EventSender {
    tx: mpsc::Sender<EventEnvelope>,
    scope: EventScope,
    record: bool,
}

impl EventSender {
    fn new(tx: mpsc::Sender<EventEnvelope>) -> Self {
        Self {
            tx,
            scope: EventScope::default(),
            record: true,
        }
    }

    pub fn with_host(&self, host: impl Into<String>) -> Self {
        self.with_scope(EventScope {
            host: Some(host.into()),
            phase: None,
        })
    }

    pub fn with_phase(&self, phase: PipelinePhase) -> Self {
        self.with_scope(EventScope {
            host: None,
            phase: Some(phase),
        })
    }

    pub fn with_scope(&self, overlay: EventScope) -> Self {
        Self {
            tx: self.tx.clone(),
            scope: self.scope.merge(overlay),
            record: self.record,
        }
    }

    pub fn unrecorded(&self) -> Self {
        let mut sender = self.clone();
        sender.record = false;
        sender
    }

    pub fn scope(&self) -> &EventScope {
        &self.scope
    }

    pub async fn send(&self, event: Event) -> Result<()> {
        if self.record {
            crate::recording::write(
                serde_json::json!({"kind": "event", "envelope": EventEnvelope { scope: self.scope.clone(), event: event.clone() }}),
            );
        }
        self.tx
            .send(EventEnvelope {
                scope: self.scope.clone(),
                event,
            })
            .await
            .context("event bus closed")
    }
}

pub struct EventStream {
    rx: mpsc::Receiver<EventEnvelope>,
}

impl EventStream {
    pub async fn recv(&mut self) -> Option<EventEnvelope> {
        self.rx.recv().await
    }
}

pub struct EventBus {
    sender: EventSender,
}

impl EventBus {
    pub fn new(capacity: usize) -> (Self, EventStream) {
        let (tx, rx) = mpsc::channel(capacity);
        (
            Self {
                sender: EventSender::new(tx),
            },
            EventStream { rx },
        )
    }

    pub fn sender(&self) -> EventSender {
        self.sender.clone()
    }
}
