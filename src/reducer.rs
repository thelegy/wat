use std::collections::{BTreeMap, HashMap, HashSet};

use crate::event::{
    BackendEvent, EventEnvelope, HostEvent, LixEvent, LixField, PipelineEvent, PipelinePhase,
};

const ACT_COPY_PATH: u64 = 100;
const ACT_FILE_TRANSFER: u64 = 101;
const ACT_COPY_PATHS: u64 = 103;
const ACT_BUILDS: u64 = 104;
const ACT_BUILD: u64 = 105;
const ACT_OPTIMISE_STORE: u64 = 106;
const ACT_VERIFY_PATHS: u64 = 107;
const ACT_SUBSTITUTE: u64 = 108;
const ACT_QUERY_PATH_INFO: u64 = 109;
const ACT_POST_BUILD_HOOK: u64 = 110;
const RES_FILE_LINKED: u64 = 100;
const RES_BUILD_LOG_LINE: u64 = 101;
const RES_UNTRUSTED_PATH: u64 = 102;
const RES_CORRUPTED_PATH: u64 = 103;
const RES_SET_PHASE: u64 = 104;
const RES_PROGRESS: u64 = 105;
const RES_SET_EXPECTED: u64 = 106;
const RES_POST_BUILD_LOG_LINE: u64 = 107;

const MIB: f64 = 1024.0 * 1024.0;

#[derive(Debug, Clone, Default)]
pub struct PipelineSnapshot {
    pub summary: String,
    pub hosts: Vec<HostSnapshot>,
    pub activities: Vec<ActivitySnapshot>,
    pub finished: bool,
}

#[derive(Debug, Clone, Default)]
pub struct HostSnapshot {
    pub host: String,
    pub message: String,
    pub phase: Option<PipelinePhase>,
    pub finished: bool,
    pub success: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct ActivitySnapshot {
    pub id: u64,
    pub host: Option<String>,
    pub text: String,
    pub last_line: String,
    pub visible: bool,
    pub done: u64,
    pub expected: u64,
    pub running: u64,
    pub failed: u64,
}

#[derive(Default)]
struct HostState {
    message: String,
    phase: Option<PipelinePhase>,
    finished: bool,
    success: Option<bool>,
}

#[derive(Default)]
struct ActivitiesByType {
    active_ids: HashSet<u64>,
    done: u64,
    expected: u64,
    failed: u64,
}

#[derive(Default)]
struct ActivityAggregate {
    running: u64,
    done: u64,
    expected: u64,
    failed: u64,
}

struct ActivityInfo {
    kind: u64,
    parent: u64,
    text: String,
    last_line: String,
    phase: String,
    done: u64,
    expected: u64,
    running: u64,
    failed: u64,
    visible: bool,
    expected_by_type: HashMap<u64, u64>,
    name: Option<String>,
    host: Option<String>,
}

pub struct PipelineReducer {
    hosts: BTreeMap<String, HostState>,
    activities: HashMap<u64, ActivityInfo>,
    by_type: HashMap<u64, ActivitiesByType>,
    files_linked: u64,
    bytes_linked: u64,
    corrupted_paths: u64,
    untrusted_paths: u64,
    finished: bool,
}

impl PipelineReducer {
    pub fn new() -> Self {
        Self {
            hosts: BTreeMap::new(),
            activities: HashMap::new(),
            by_type: HashMap::new(),
            files_linked: 0,
            bytes_linked: 0,
            corrupted_paths: 0,
            untrusted_paths: 0,
            finished: false,
        }
    }

    pub fn apply(&mut self, envelope: &EventEnvelope) {
        match &envelope.event {
            crate::event::Event::Pipeline(event) => self.apply_pipeline(event),
            crate::event::Event::Host(event) => self.apply_host(event),
            crate::event::Event::Backend(event) => self.apply_backend(&envelope.scope.host, event),
            crate::event::Event::Log(_) => {}
            crate::event::Event::Artifact(_) => {}
        }
    }

    pub fn snapshot(&self) -> PipelineSnapshot {
        PipelineSnapshot {
            summary: self.status_summary(),
            hosts: self
                .hosts
                .iter()
                .map(|(host, state)| HostSnapshot {
                    host: host.clone(),
                    message: state.message.clone(),
                    phase: state.phase,
                    finished: state.finished,
                    success: state.success,
                })
                .collect(),
            activities: self
                .activities
                .iter()
                .map(|(id, activity)| ActivitySnapshot {
                    id: *id,
                    host: activity.host.clone(),
                    text: activity.text.clone(),
                    last_line: activity.last_line.clone(),
                    visible: activity.visible,
                    done: activity.done,
                    expected: activity.expected,
                    running: activity.running,
                    failed: activity.failed,
                })
                .collect(),
            finished: self.finished,
        }
    }

    fn apply_pipeline(&mut self, event: &PipelineEvent) {
        match event {
            PipelineEvent::Started { hosts } => {
                for host in hosts {
                    self.hosts.entry(host.clone()).or_default();
                }
            }
            PipelineEvent::PhaseStarted(phase) => {
                let _ = phase;
            }
            PipelineEvent::PhaseFinished { phase, success } => {
                let _ = (phase, success);
            }
            PipelineEvent::Finished { success } => self.finished = *success,
        }
    }

    fn apply_host(&mut self, event: &HostEvent) {
        match event {
            HostEvent::Started { host, phase } => {
                let state = self.hosts.entry(host.clone()).or_default();
                state.phase = Some(*phase);
                state.finished = false;
                state.success = None;
                state.message = format!("{} {}", phase_name(*phase), host);
            }
            HostEvent::Finished {
                host,
                phase,
                success,
            } => {
                let state = self.hosts.entry(host.clone()).or_default();
                state.phase = Some(*phase);
                state.finished = true;
                state.success = Some(*success);
                state.message = if *success {
                    format!("{} {} complete", phase_name(*phase), host)
                } else {
                    format!("{} {} failed", phase_name(*phase), host)
                };
            }
        }
    }

    fn apply_backend(&mut self, host: &Option<String>, event: &BackendEvent) {
        match event {
            BackendEvent::Lix(event) => self.apply_lix(host, event),
        }
    }

    fn apply_lix(&mut self, host: &Option<String>, event: &LixEvent) {
        match event {
            LixEvent::Start {
                id,
                kind,
                parent,
                level,
                text,
                fields,
            } => self.handle_start(*id, *kind, *parent, *level, text, fields, host.clone()),
            LixEvent::Stop { id } => self.handle_stop(*id),
            LixEvent::Result {
                id,
                result_type,
                fields,
            } => self.handle_result(*id, *result_type, fields),
        }
    }

    fn handle_start(
        &mut self,
        id: u64,
        kind: u64,
        parent: u64,
        level: u64,
        text: &str,
        fields: &[LixField],
        host: Option<String>,
    ) {
        let _ = level;
        if id == 0 {
            return;
        }

        let mut activity = ActivityInfo {
            kind,
            parent,
            text: text.to_string(),
            last_line: String::new(),
            phase: String::new(),
            done: 0,
            expected: 0,
            running: 0,
            failed: 0,
            visible: true,
            expected_by_type: HashMap::new(),
            name: None,
            host,
        };

        match kind {
            ACT_BUILD => {
                let name = fields
                    .first()
                    .and_then(LixField::as_str)
                    .map(store_path_to_name)
                    .unwrap_or_default();
                let machine = fields.get(1).and_then(LixField::as_str).unwrap_or_default();

                if !name.is_empty() {
                    activity.text = format!("building {name}");
                }
                if !machine.is_empty() {
                    activity.text.push_str(&format!(" on {machine}"));
                }
                activity.name = if name.is_empty() { None } else { Some(name) };
            }
            ACT_SUBSTITUTE => {
                let name = fields
                    .first()
                    .and_then(LixField::as_str)
                    .map(store_path_to_name)
                    .unwrap_or_default();
                let substituter = fields.get(1).and_then(LixField::as_str).unwrap_or_default();

                if substituter.starts_with("local") {
                    activity.text = format!("copying {name} from {substituter}");
                } else {
                    activity.text = format!("fetching {name} from {substituter}");
                }
            }
            ACT_POST_BUILD_HOOK => {
                let name = fields
                    .first()
                    .and_then(LixField::as_str)
                    .map(store_path_to_name)
                    .unwrap_or_default();
                activity.text = format!("post-build {name}");
                activity.name = if name.is_empty() { None } else { Some(name) };
            }
            ACT_QUERY_PATH_INFO => {
                let name = fields
                    .first()
                    .and_then(LixField::as_str)
                    .map(store_path_to_name)
                    .unwrap_or_default();
                let substituter = fields.get(1).and_then(LixField::as_str).unwrap_or_default();
                activity.text = format!("querying {name} on {substituter}");
            }
            _ => {}
        }

        if (kind == ACT_FILE_TRANSFER
            && (self.has_ancestor(parent, ACT_COPY_PATH)
                || self.has_ancestor(parent, ACT_QUERY_PATH_INFO)))
            || (kind == ACT_COPY_PATH && self.has_ancestor(parent, ACT_SUBSTITUTE))
        {
            activity.visible = false;
        }

        self.activities.insert(id, activity);
        self.by_type.entry(kind).or_default().active_ids.insert(id);
    }

    fn handle_stop(&mut self, id: u64) {
        let Some(activity) = self.activities.remove(&id) else {
            return;
        };

        let aggregate = self.by_type.entry(activity.kind).or_default();
        aggregate.done = aggregate.done.saturating_add(activity.done);
        aggregate.failed = aggregate.failed.saturating_add(activity.failed);
        aggregate.active_ids.remove(&id);

        for (kind, expected) in activity.expected_by_type {
            let by_type = self.by_type.entry(kind).or_default();
            by_type.expected = by_type.expected.saturating_sub(expected);
        }
    }

    fn handle_result(&mut self, id: u64, result_type: u64, fields: &[LixField]) {
        match result_type {
            RES_FILE_LINKED => {
                self.files_linked = self.files_linked.saturating_add(1);
                if let Some(bytes) = fields.first().and_then(LixField::as_int) {
                    self.bytes_linked = self.bytes_linked.saturating_add(bytes);
                }
            }
            RES_BUILD_LOG_LINE | RES_POST_BUILD_LOG_LINE => {
                let Some(line) = fields.first().and_then(LixField::as_str) else {
                    return;
                };

                let line = line.trim().to_string();
                if line.is_empty() {
                    return;
                }

                let mut name = String::new();
                if let Some(activity) = self.activities.get_mut(&id) {
                    activity.last_line = line.clone();
                    if let Some(activity_name) = &activity.name {
                        name = activity_name.clone();
                    }
                }

                let _ = name;
            }
            RES_UNTRUSTED_PATH => {
                self.untrusted_paths = self.untrusted_paths.saturating_add(1);
            }
            RES_CORRUPTED_PATH => {
                self.corrupted_paths = self.corrupted_paths.saturating_add(1);
            }
            RES_SET_PHASE => {
                let Some(phase) = fields.first().and_then(LixField::as_str) else {
                    return;
                };

                if let Some(activity) = self.activities.get_mut(&id) {
                    activity.phase = phase.to_string();
                }
            }
            RES_PROGRESS => {
                if let Some(activity) = self.activities.get_mut(&id) {
                    activity.done = fields.first().and_then(LixField::as_int).unwrap_or(0);
                    activity.expected = fields.get(1).and_then(LixField::as_int).unwrap_or(0);
                    activity.running = fields.get(2).and_then(LixField::as_int).unwrap_or(0);
                    activity.failed = fields.get(3).and_then(LixField::as_int).unwrap_or(0);
                }
            }
            RES_SET_EXPECTED => {
                let Some(expected_kind) = fields.first().and_then(LixField::as_int) else {
                    return;
                };
                let expected_value = fields.get(1).and_then(LixField::as_int).unwrap_or(0);

                if let Some(activity) = self.activities.get_mut(&id) {
                    let entry = activity.expected_by_type.entry(expected_kind).or_insert(0);
                    let old = *entry;
                    *entry = expected_value;

                    let aggregate = self.by_type.entry(expected_kind).or_default();
                    aggregate.expected = aggregate
                        .expected
                        .saturating_sub(old)
                        .saturating_add(expected_value);
                }
            }
            _ => {}
        }
    }

    fn has_ancestor(&self, mut current: u64, wanted_kind: u64) -> bool {
        while current != 0 {
            let Some(activity) = self.activities.get(&current) else {
                return false;
            };

            if activity.kind == wanted_kind {
                return true;
            }

            current = activity.parent;
        }

        false
    }

    fn status_summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(rendered) = self.render_count_activity(ACT_BUILDS, "built") {
            parts.push(rendered);
        }

        let copied = self.render_count_activity(ACT_COPY_PATHS, "copied");
        let copied_size = self.render_mib_activity(ACT_COPY_PATH, "MiB");
        if copied.is_some() || copied_size.is_some() {
            let mut section = copied.unwrap_or_else(|| "0 copied".to_string());
            if let Some(size) = copied_size {
                section.push_str(" (");
                section.push_str(&size);
                section.push(')');
            }
            parts.push(section);
        }

        if let Some(downloaded) = self.render_mib_activity(ACT_FILE_TRANSFER, "MiB DL") {
            parts.push(downloaded);
        }

        if let Some(optimized) = self.render_count_activity(ACT_OPTIMISE_STORE, "paths optimised") {
            parts.push(format!(
                "{optimized}, {:.1} MiB / {} inodes freed",
                self.bytes_linked as f64 / MIB,
                self.files_linked
            ));
        }

        if let Some(verified) = self.render_count_activity(ACT_VERIFY_PATHS, "paths verified") {
            parts.push(verified);
        }

        if self.corrupted_paths > 0 {
            parts.push(format!("{} corrupted", self.corrupted_paths));
        }

        if self.untrusted_paths > 0 {
            parts.push(format!("{} untrusted", self.untrusted_paths));
        }

        parts.join(", ")
    }

    fn render_count_activity(&self, kind: u64, label: &str) -> Option<String> {
        let aggregate = self.aggregate(kind);
        if aggregate.running == 0
            && aggregate.done == 0
            && aggregate.expected == 0
            && aggregate.failed == 0
        {
            return None;
        }

        let mut rendered = if aggregate.running > 0 {
            if aggregate.expected > 0 {
                format!(
                    "{}/{}/{} {}",
                    aggregate.running, aggregate.done, aggregate.expected, label
                )
            } else {
                format!("{}/{} {}", aggregate.running, aggregate.done, label)
            }
        } else if aggregate.expected != aggregate.done {
            if aggregate.expected > 0 {
                format!("{}/{} {}", aggregate.done, aggregate.expected, label)
            } else {
                format!("{} {}", aggregate.done, label)
            }
        } else {
            format!("{} {}", aggregate.done, label)
        };

        if aggregate.failed > 0 {
            rendered.push_str(&format!(" ({} failed)", aggregate.failed));
        }

        Some(rendered)
    }

    fn render_mib_activity(&self, kind: u64, label: &str) -> Option<String> {
        let aggregate = self.aggregate(kind);
        if aggregate.running == 0
            && aggregate.done == 0
            && aggregate.expected == 0
            && aggregate.failed == 0
        {
            return None;
        }

        let mut rendered = if aggregate.running > 0 {
            if aggregate.expected > 0 {
                format!(
                    "{:.1}/{:.1}/{:.1} {}",
                    aggregate.running as f64 / MIB,
                    aggregate.done as f64 / MIB,
                    aggregate.expected as f64 / MIB,
                    label
                )
            } else {
                format!(
                    "{:.1}/{:.1} {}",
                    aggregate.running as f64 / MIB,
                    aggregate.done as f64 / MIB,
                    label
                )
            }
        } else if aggregate.expected != aggregate.done {
            if aggregate.expected > 0 {
                format!(
                    "{:.1}/{:.1} {}",
                    aggregate.done as f64 / MIB,
                    aggregate.expected as f64 / MIB,
                    label
                )
            } else {
                format!("{:.1} {}", aggregate.done as f64 / MIB, label)
            }
        } else {
            format!("{:.1} {}", aggregate.done as f64 / MIB, label)
        };

        if aggregate.failed > 0 {
            rendered.push_str(&format!(" ({:.1} failed)", aggregate.failed as f64 / MIB));
        }

        Some(rendered)
    }

    fn aggregate(&self, kind: u64) -> ActivityAggregate {
        let mut aggregate = ActivityAggregate::default();
        if let Some(by_type) = self.by_type.get(&kind) {
            aggregate.done = by_type.done;
            aggregate.expected = by_type.done;
            aggregate.failed = by_type.failed;

            for activity_id in &by_type.active_ids {
                if let Some(activity) = self.activities.get(activity_id) {
                    aggregate.done = aggregate.done.saturating_add(activity.done);
                    aggregate.expected = aggregate.expected.saturating_add(activity.expected);
                    aggregate.running = aggregate.running.saturating_add(activity.running);
                    aggregate.failed = aggregate.failed.saturating_add(activity.failed);
                }
            }

            aggregate.expected = aggregate.expected.max(by_type.expected);
        }

        aggregate
    }
}

impl LixField {
    fn as_int(&self) -> Option<u64> {
        match self {
            LixField::Int(value) => Some(*value),
            LixField::Str(_) => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            LixField::Int(_) => None,
            LixField::Str(value) => Some(value.as_str()),
        }
    }
}

impl Default for PipelineReducer {
    fn default() -> Self {
        Self::new()
    }
}

fn phase_name(phase: PipelinePhase) -> &'static str {
    match phase {
        PipelinePhase::Evaluate => "evaluating",
        PipelinePhase::Build => "building",
        PipelinePhase::Deploy => "deploying",
    }
}

fn store_path_to_name(path: &str) -> String {
    let Some(base) = path.rsplit('/').next() else {
        return String::new();
    };

    let Some((_, name)) = base.split_once('-') else {
        return String::new();
    };

    name.trim_end_matches(".drv").to_string()
}
