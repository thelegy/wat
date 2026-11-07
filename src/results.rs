use std::{collections::BTreeMap, io::Write, path::Path, sync::{Arc, Mutex}};

use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Clone, Default)]
pub(crate) struct Report(Arc<Mutex<Data>>);

#[derive(Serialize)]
struct Data {
    version: u32,
    status: &'static str,
    hosts: BTreeMap<String, Host>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl Default for Data {
    fn default() -> Self {
        Self { version: 1, status: "incomplete", hosts: BTreeMap::new(), reason: None }
    }
}

#[derive(Serialize)]
struct Host {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl Report {
    pub fn select(&self, hosts: impl Iterator<Item = String>) {
        let mut data = self.0.lock().unwrap();
        data.hosts = hosts.map(|host| (host, Host { status: "incomplete", reason: None })).collect();
    }

    pub fn host_finished<T>(&self, host: String, result: &Result<T>) {
        self.0.lock().unwrap().hosts.insert(host, Host {
            status: if result.is_ok() { "success" } else { "build-failed" },
            reason: result.as_ref().err().map(|error| format!("{error:#}")),
        });
    }

    pub fn finish(&self, result: &Result<()>) {
        let mut data = self.0.lock().unwrap();
        let complete = (result.is_ok() || result.as_ref().err().is_some_and(crate::operations::failure_reported))
            && !data.hosts.is_empty()
            && data.hosts.values().all(|host| host.status != "incomplete");
        data.status = if complete { "complete" } else { "incomplete" };
        if !complete {
            data.reason = Some(result.as_ref().err().map(|error| format!("{error:#}"))
                .unwrap_or_else(|| "not all hosts completed".into()));
        }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        (|| -> Result<()> {
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            serde_json::to_writer_pretty(&mut file, &*self.0.lock().unwrap())?;
            file.write_all(b"\n")?;
            file.as_file().sync_all()?;
            file.persist(path).map_err(|error| error.error)?;
            Ok(())
        })().with_context(|| format!("failed to write results file {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfinished_or_fatal_runs_cannot_be_complete() {
        let report = Report::default();
        report.select(["z".into(), "a".into()].into_iter());
        report.host_finished("a".into(), &Ok(()));
        report.finish(&Err(anyhow::anyhow!("worker failed")));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.json");
        report.write(&path).unwrap();
        let first = std::fs::read(&path).unwrap();
        let data: serde_json::Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(data["status"], "incomplete");
        assert_eq!(data["hosts"]["z"]["status"], "incomplete");
        assert_eq!(data["reason"], "worker failed");
        report.write(&path).unwrap();
        assert_eq!(first, std::fs::read(path).unwrap());
        assert!(report.write(dir.path()).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
