use tokio::process::Command;

use crate::config::Connection;

/// Commands and store access for a configured deployment target.
pub trait Target: Send + Sync {
    fn command(&self, args: &[&str], non_interactive: bool) -> Command;

    /// Store URI for closure uploads, or None when the target shares the local store.
    fn store_destination(&self) -> Option<String>;

    /// Flush filesystem writes on the target after activation.
    fn sync_command(&self) -> Command;
}

pub fn target(hostname: &str, connection: Connection) -> Box<dyn Target> {
    match connection {
        Connection::Ssh => Box::new(Ssh { host: format!("root@{hostname}") }),
        Connection::Sudo => Box::new(Sudo),
    }
}

struct Ssh {
    host: String,
}

impl Target for Ssh {
    fn command(&self, args: &[&str], non_interactive: bool) -> Command {
        let mut command = Command::new("ssh");
        if non_interactive {
            command.args(["-o", "BatchMode=yes"]);
        }
        command.arg(&self.host).arg(args.join(" "));
        command
    }

    fn store_destination(&self) -> Option<String> {
        Some(format!("ssh://{}", self.host))
    }

    fn sync_command(&self) -> Command {
        self.command(&["sync"], false)
    }
}

struct Sudo;

impl Target for Sudo {
    fn command(&self, args: &[&str], non_interactive: bool) -> Command {
        let mut command = Command::new("sudo");
        if non_interactive {
            command.arg("-n");
        }
        command.args(args);
        command
    }

    fn store_destination(&self) -> Option<String> {
        None
    }

    fn sync_command(&self) -> Command {
        Command::new("sync")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation(command: Command) -> (String, Vec<String>) {
        let command = command.as_std();
        (
            command.get_program().to_string_lossy().into_owned(),
            command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect(),
        )
    }

    #[test]
    fn ssh_routes_commands_and_store_access_to_the_same_host() {
        let target = target("example", Connection::Ssh);
        assert_eq!(target.store_destination().as_deref(), Some("ssh://root@example"));
        assert_eq!(invocation(target.command(&["readlink", "-e", "/run/current-system"], true)),
            ("ssh".into(), vec!["-o".into(), "BatchMode=yes".into(), "root@example".into(), "readlink -e /run/current-system".into()]));
        assert_eq!(invocation(target.command(&["systemctl", "reboot"], false)),
            ("ssh".into(), vec!["root@example".into(), "systemctl reboot".into()]));
        assert_eq!(invocation(target.sync_command()),
            ("ssh".into(), vec!["root@example".into(), "sync".into()]));
    }

    #[test]
    fn sudo_runs_locally_and_only_probes_disable_prompts() {
        let target = target("example", Connection::Sudo);
        assert_eq!(target.store_destination(), None);
        assert_eq!(invocation(target.command(&["readlink", "-e", "/run/current-system"], true)),
            ("sudo".into(), vec!["-n".into(), "readlink".into(), "-e".into(), "/run/current-system".into()]));
        assert_eq!(invocation(target.command(&["systemctl", "reboot"], false)),
            ("sudo".into(), vec!["systemctl".into(), "reboot".into()]));
        assert_eq!(invocation(target.sync_command()), ("sync".into(), vec![]));
    }
}
