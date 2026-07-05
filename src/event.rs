use std::path::PathBuf;

use anyhow::{Context, Result};
use log::Level;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct EventEnvelope {
    pub scope: EventScope,
    pub event: Event,
}

#[derive(Debug, Clone)]
pub enum Event {
    Pipeline(PipelineEvent),
    Host(HostEvent),
    Backend(BackendEvent),
    Log(LogEvent),
    Artifact(ArtifactEvent),
}

#[derive(Debug, Clone)]
pub enum PipelineEvent {
    Started { hosts: Vec<String> },
    PhaseStarted(PipelinePhase),
    PhaseFinished { phase: PipelinePhase, success: bool },
    Finished { success: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelinePhase {
    Evaluate,
    Build,
    Deploy,
}

#[derive(Debug, Clone)]
pub enum HostEvent {
    Started { host: String, phase: PipelinePhase },
    Finished { host: String, phase: PipelinePhase, success: bool },
}

#[derive(Debug, Clone)]
pub enum ArtifactEvent {
    Ready { host: String, path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct LogEvent {
    pub level: Level,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum BackendEvent {
    Lix(LixEvent),
}

#[derive(Debug, Clone)]
pub enum LixEvent {
    Start {
        id: u64,
        kind: u64,
        parent: u64,
        level: u64,
        text: String,
        fields: Vec<LixField>,
    },
    Stop { id: u64 },
    Result {
        id: u64,
        result_type: u64,
        fields: Vec<LixField>,
    },
}

#[derive(Debug, Clone)]
pub enum LixField {
    Int(u64),
    Str(String),
}

#[derive(Clone)]
pub struct EventSender {
    tx: mpsc::Sender<EventEnvelope>,
    scope: EventScope,
}

impl EventSender {
    fn new(tx: mpsc::Sender<EventEnvelope>) -> Self {
        Self {
            tx,
            scope: EventScope::default(),
        }
    }

    pub fn with_host(&self, host: impl Into<String>) -> Self {
        self.with_overlay(EventScope {
            host: Some(host.into()),
            phase: None,
        })
    }

    pub fn with_phase(&self, phase: PipelinePhase) -> Self {
        self.with_overlay(EventScope {
            host: None,
            phase: Some(phase),
        })
    }

    fn with_overlay(&self, overlay: EventScope) -> Self {
        Self {
            tx: self.tx.clone(),
            scope: self.scope.merge(overlay),
        }
    }

    pub async fn send(&self, event: Event) -> Result<()> {
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
