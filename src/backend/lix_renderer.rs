use std::{
    collections::{HashMap, HashSet},
    io::IsTerminal,
    time::{Duration, Instant},
};

use indicatif::{ProgressBar, ProgressStyle};
use log::{Level, debug, error, info, trace, warn};
use serde_json::Value;

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
const ACT_BUILD_WAITING: u64 = 111;

const RES_FILE_LINKED: u64 = 100;
const RES_BUILD_LOG_LINE: u64 = 101;
const RES_UNTRUSTED_PATH: u64 = 102;
const RES_CORRUPTED_PATH: u64 = 103;
const RES_SET_PHASE: u64 = 104;
const RES_PROGRESS: u64 = 105;
const RES_SET_EXPECTED: u64 = 106;
const RES_POST_BUILD_LOG_LINE: u64 = 107;

const MIB: f64 = 1024.0 * 1024.0;

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
    order: u64,
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
    start_time: Instant,
}

enum Field {
    Int(u64),
    Str(String),
}

impl Field {
    fn as_int(&self) -> Option<u64> {
        match self {
            Field::Int(value) => Some(*value),
            Field::Str(_) => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Field::Int(_) => None,
            Field::Str(value) => Some(value.as_str()),
        }
    }
}

pub struct LixRenderer {
    progress: Option<ProgressBar>,
    last_status: String,
    sequence: u64,
    activities: HashMap<u64, ActivityInfo>,
    by_type: HashMap<u64, ActivitiesByType>,
    files_linked: u64,
    bytes_linked: u64,
    corrupted_paths: u64,
    untrusted_paths: u64,
}

impl LixRenderer {
    pub fn new() -> Self {
        let progress = if std::io::stderr().is_terminal() {
            let progress = ProgressBar::new_spinner();
            let style = ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner());
            progress.set_style(style);
            progress.enable_steady_tick(Duration::from_millis(120));
            Some(progress)
        } else {
            None
        };

        Self {
            progress,
            last_status: String::new(),
            sequence: 0,
            activities: HashMap::new(),
            by_type: HashMap::new(),
            files_linked: 0,
            bytes_linked: 0,
            corrupted_paths: 0,
            untrusted_paths: 0,
        }
    }

    pub fn handle_stderr_line(&mut self, line: &str) {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            return;
        }

        if let Some(json_line) = line.strip_prefix("@nix ") {
            match serde_json::from_str::<Value>(json_line) {
                Ok(value) => self.handle_json(value),
                Err(error) => self.log(Level::Warn, &format!("invalid nix log JSON: {error}")),
            }
            return;
        }

        self.log(Level::Info, line);
    }

    pub fn finish(&mut self, success: bool) {
        let Some(progress) = self.progress.take() else {
            return;
        };

        if success {
            progress.finish_and_clear();
            return;
        }

        let final_message = self.current_status_line();
        if final_message.is_empty() {
            progress.abandon_with_message("command failed");
        } else {
            progress.abandon_with_message(final_message);
        }
    }

    fn handle_json(&mut self, value: Value) {
        let Some(action) = value.get("action").and_then(Value::as_str) else {
            return;
        };

        match action {
            "msg" => {
                let level = value.get("level").and_then(Value::as_u64).unwrap_or(3);
                if let Some(message) = value.get("msg").and_then(Value::as_str) {
                    self.log(self.level_for_verbosity(level), message);
                }
            }
            "start" => self.handle_start(&value),
            "stop" => self.handle_stop(&value),
            "result" => self.handle_result(&value),
            _ => {}
        }
    }

    fn handle_start(&mut self, value: &Value) {
        let Some(id) = value.get("id").and_then(Value::as_u64) else {
            return;
        };

        let kind = value.get("type").and_then(Value::as_u64).unwrap_or(0);
        let level = value.get("level").and_then(Value::as_u64).unwrap_or(3);
        let parent = value.get("parent").and_then(Value::as_u64).unwrap_or(0);
        let text = value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        if !text.is_empty() && kind != ACT_BUILD_WAITING {
            self.log(self.level_for_verbosity(level), &format!("{text}..."));
        }

        let fields = parse_fields(value.get("fields"));

        self.sequence += 1;

        let mut activity = ActivityInfo {
            order: self.sequence,
            kind,
            parent,
            text,
            last_line: String::new(),
            phase: String::new(),
            done: 0,
            expected: 0,
            running: 0,
            failed: 0,
            visible: true,
            expected_by_type: HashMap::new(),
            name: None,
            start_time: Instant::now(),
        };

        match kind {
            ACT_BUILD => {
                let name = fields
                    .first()
                    .and_then(Field::as_str)
                    .map(store_path_to_name)
                    .unwrap_or_default();
                let machine = fields.get(1).and_then(Field::as_str).unwrap_or_default();

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
                    .and_then(Field::as_str)
                    .map(store_path_to_name)
                    .unwrap_or_default();
                let substituter = fields.get(1).and_then(Field::as_str).unwrap_or_default();

                if substituter.starts_with("local") {
                    activity.text = format!("copying {name} from {substituter}");
                } else {
                    activity.text = format!("fetching {name} from {substituter}");
                }
            }
            ACT_POST_BUILD_HOOK => {
                let name = fields
                    .first()
                    .and_then(Field::as_str)
                    .map(store_path_to_name)
                    .unwrap_or_default();
                activity.text = format!("post-build {name}");
                activity.name = if name.is_empty() { None } else { Some(name) };
            }
            ACT_QUERY_PATH_INFO => {
                let name = fields
                    .first()
                    .and_then(Field::as_str)
                    .map(store_path_to_name)
                    .unwrap_or_default();
                let substituter = fields.get(1).and_then(Field::as_str).unwrap_or_default();
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
        self.refresh_status();
    }

    fn handle_stop(&mut self, value: &Value) {
        let Some(id) = value.get("id").and_then(Value::as_u64) else {
            return;
        };

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

        self.refresh_status();
    }

    fn handle_result(&mut self, value: &Value) {
        let Some(id) = value.get("id").and_then(Value::as_u64) else {
            return;
        };
        let Some(result_type) = value.get("type").and_then(Value::as_u64) else {
            return;
        };

        let fields = parse_fields(value.get("fields"));

        match result_type {
            RES_FILE_LINKED => {
                self.files_linked = self.files_linked.saturating_add(1);
                if let Some(bytes) = fields.first().and_then(Field::as_int) {
                    self.bytes_linked = self.bytes_linked.saturating_add(bytes);
                }
                self.refresh_status();
            }
            RES_BUILD_LOG_LINE | RES_POST_BUILD_LOG_LINE => {
                let Some(line) = fields.first().and_then(Field::as_str) else {
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

                if result_type == RES_POST_BUILD_LOG_LINE {
                    self.log(Level::Info, &format!("post-build-hook: {line}"));
                } else if name.is_empty() {
                    self.log(Level::Info, &line);
                } else {
                    self.log(Level::Info, &format!("{name}> {line}"));
                }

                self.refresh_status();
            }
            RES_UNTRUSTED_PATH => {
                self.untrusted_paths = self.untrusted_paths.saturating_add(1);
                self.refresh_status();
            }
            RES_CORRUPTED_PATH => {
                self.corrupted_paths = self.corrupted_paths.saturating_add(1);
                self.refresh_status();
            }
            RES_SET_PHASE => {
                let Some(phase) = fields.first().and_then(Field::as_str) else {
                    return;
                };

                if let Some(activity) = self.activities.get_mut(&id) {
                    activity.phase = phase.to_string();
                    self.refresh_status();
                }
            }
            RES_PROGRESS => {
                if let Some(activity) = self.activities.get_mut(&id) {
                    activity.done = fields.first().and_then(Field::as_int).unwrap_or(0);
                    activity.expected = fields.get(1).and_then(Field::as_int).unwrap_or(0);
                    activity.running = fields.get(2).and_then(Field::as_int).unwrap_or(0);
                    activity.failed = fields.get(3).and_then(Field::as_int).unwrap_or(0);
                    self.refresh_status();
                }
            }
            RES_SET_EXPECTED => {
                let Some(expected_kind) = fields.first().and_then(Field::as_int) else {
                    return;
                };
                let expected_value = fields.get(1).and_then(Field::as_int).unwrap_or(0);

                if let Some(activity) = self.activities.get_mut(&id) {
                    let entry = activity.expected_by_type.entry(expected_kind).or_insert(0);
                    let old = *entry;
                    *entry = expected_value;

                    let aggregate = self.by_type.entry(expected_kind).or_default();
                    aggregate.expected = aggregate
                        .expected
                        .saturating_sub(old)
                        .saturating_add(expected_value);

                    self.refresh_status();
                }
            }
            _ => {}
        }
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

    fn refresh_status(&mut self) {
        let Some(progress) = self.progress.as_ref() else {
            return;
        };

        let line = self.current_status_line();
        if line != self.last_status {
            progress.set_message(line.clone());
            self.last_status = line;
        }

        progress.tick();
    }

    fn current_status_line(&self) -> String {
        let status = self.status_summary();
        let activity = self.current_activity();

        match (status.is_empty(), activity.is_empty()) {
            (false, false) => format!("[{status}] {activity}"),
            (false, true) => format!("[{status}]"),
            (true, false) => activity,
            (true, true) => String::new(),
        }
    }

    fn current_activity(&self) -> String {
        let Some(activity) = self
            .activities
            .values()
            .filter(|activity| activity.visible)
            .filter(|activity| !activity.text.is_empty() || !activity.last_line.is_empty())
            .filter(|activity| activity.start_time.elapsed() >= Duration::from_millis(10))
            .max_by_key(|activity| activity.order)
        else {
            return String::new();
        };

        let mut line = activity.text.clone();

        if !activity.phase.is_empty() {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push('(');
            line.push_str(&activity.phase);
            line.push(')');
        }

        if !activity.last_line.is_empty() {
            if !line.is_empty() {
                line.push_str(": ");
            }
            line.push_str(&activity.last_line);
        }

        line
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

    fn log(&self, level: Level, message: &str) {
        if let Some(progress) = &self.progress {
            progress.suspend(|| log_message(level, message));
            return;
        }

        log_message(level, message);
    }
}

fn log_message(level: Level, message: &str) {
    match level {
        Level::Error => error!("{message}"),
        Level::Warn => warn!("{message}"),
        Level::Info => info!("{message}"),
        Level::Debug => debug!("{message}"),
        Level::Trace => trace!("{message}"),
    }
}

fn parse_fields(value: Option<&Value>) -> Vec<Field> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| match item {
            Value::Number(number) => number.as_u64().map(Field::Int),
            Value::String(value) => Some(Field::Str(value.clone())),
            _ => None,
        })
        .collect()
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
