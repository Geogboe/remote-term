use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use super::RunConfig;
use crate::platform::lan_ip;

const REGISTRY_VERSION: u32 = 1;
const STARTUP_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub version: u32,
    pub id: String,
    pub pid: u32,
    pub started_at_unix: u64,
    pub program: String,
    pub writable: bool,
    pub local_url: String,
    pub lan_url: Option<String>,
}

impl SessionRecord {
    fn from_config(config: &RunConfig) -> Self {
        let port = config.bind_addr.port();
        let local_url = lan_ip::terminal_url(
            lan_ip::local_access_ip(config.bind_addr.ip()),
            port,
            &config.token,
        );
        let lan_url = config
            .lan
            .then(|| {
                lan_ip::primary_lan_ip().map(|ip| lan_ip::terminal_url(ip, port, &config.token))
            })
            .flatten();
        let pid = std::process::id();

        Self {
            version: REGISTRY_VERSION,
            id: pid.to_string(),
            pid,
            started_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            program: config.command.first().cloned().unwrap_or_default(),
            writable: config.web_write,
            local_url,
            lan_url,
        }
    }
}

pub struct Registration {
    path: PathBuf,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn register(config: &RunConfig) -> anyhow::Result<Registration> {
    register_in(&registry_dir()?, SessionRecord::from_config(config))
}

pub fn list() -> anyhow::Result<Vec<SessionRecord>> {
    list_in(&registry_dir()?, probe_session)
}

fn registry_dir() -> anyhow::Result<PathBuf> {
    let base = BaseDirs::new().context("could not determine the current user's data directory")?;
    Ok(base.data_local_dir().join("rterm").join("sessions"))
}

fn register_in(directory: &Path, record: SessionRecord) -> anyhow::Result<Registration> {
    fs::create_dir_all(directory)
        .with_context(|| format!("failed to create session registry {}", directory.display()))?;
    secure_directory(directory)?;

    let path = directory.join(format!("{}.json", record.id));
    let temporary = directory.join(format!(".{}.{}.tmp", record.id, rand::random::<u64>()));
    let payload = serde_json::to_vec_pretty(&record)?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    secure_file_options(&mut options);
    let mut file = options
        .open(&temporary)
        .with_context(|| format!("failed to create {}", temporary.display()))?;
    file.write_all(&payload)?;
    file.sync_all()?;
    drop(file);

    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to replace stale registry {}", path.display()))?;
    }
    fs::rename(&temporary, &path).with_context(|| {
        format!(
            "failed to publish session registry {}",
            path.as_path().display()
        )
    })?;

    Ok(Registration { path })
}

fn list_in(
    directory: &Path,
    probe: impl Fn(&SessionRecord) -> bool,
) -> anyhow::Result<Vec<SessionRecord>> {
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut active = Vec::new();
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read session registry {}", directory.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }

        let record = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<SessionRecord>(&bytes).ok());
        match record {
            Some(record)
                if record.version == REGISTRY_VERSION
                    && (probe(&record) || record_is_starting(&record)) =>
            {
                active.push(record);
            }
            _ => {
                let _ = fs::remove_file(path);
            }
        }
    }
    active.sort_by_key(|record| record.started_at_unix);
    Ok(active)
}

fn record_is_starting(record: &SessionRecord) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(record.started_at_unix) <= STARTUP_GRACE.as_secs()
}

fn probe_session(record: &SessionRecord) -> bool {
    probe_http_url(&record.local_url).unwrap_or(false)
}

fn probe_http_url(url: &str) -> anyhow::Result<bool> {
    let rest = url
        .strip_prefix("http://")
        .context("session URL is not HTTP")?;
    let (authority, path) = rest.split_once('/').context("session URL has no path")?;
    let (host, port) = authority
        .rsplit_once(':')
        .context("session URL has no port")?;
    let host = host.trim_matches(['[', ']']);
    let address = format!("{host}:{port}")
        .parse::<SocketAddr>()
        .context("session URL has an invalid address")?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(300))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    write!(
        stream,
        "GET /{path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = [0_u8; 64];
    let read = stream.read(&mut response)?;
    Ok(response[..read].starts_with(b"HTTP/1.1 200")
        || response[..read].starts_with(b"HTTP/1.0 200"))
}

#[cfg(unix)]
fn secure_directory(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to secure session registry {}", path.display()))
}

#[cfg(not(unix))]
fn secure_directory(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn secure_file_options(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.mode(0o600);
}

#[cfg(not(unix))]
fn secure_file_options(_options: &mut OpenOptions) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory() -> PathBuf {
        std::env::temp_dir().join(format!("rterm-registry-test-{}", rand::random::<u64>()))
    }

    fn record() -> SessionRecord {
        SessionRecord {
            version: REGISTRY_VERSION,
            id: "42".to_string(),
            pid: 42,
            started_at_unix: 1,
            program: "pwsh".to_string(),
            writable: true,
            local_url: "http://127.0.0.1:7843/t/example".to_string(),
            lan_url: None,
        }
    }

    #[test]
    fn registration_is_visible_and_removed_on_drop() {
        let directory = temporary_directory();
        let registration = register_in(&directory, record()).unwrap();

        let listed = list_in(&directory, |_| true).unwrap();
        assert_eq!(listed, [record()]);

        drop(registration);
        assert!(list_in(&directory, |_| true).unwrap().is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn listing_prunes_stale_sessions() {
        let directory = temporary_directory();
        let registration = register_in(&directory, record()).unwrap();

        assert!(list_in(&directory, |_| false).unwrap().is_empty());
        assert!(!registration.path.exists());

        drop(registration);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn listing_keeps_a_session_during_startup_grace() {
        let directory = temporary_directory();
        let mut starting = record();
        starting.started_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let registration = register_in(&directory, starting.clone()).unwrap();

        assert_eq!(list_in(&directory, |_| false).unwrap(), [starting]);

        drop(registration);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_urls_fail_closed() {
        assert!(!probe_http_url("not-a-url").unwrap_or(false));
    }

    #[test]
    fn record_uses_the_effective_listener_address() {
        let config = RunConfig {
            command: vec!["pwsh".to_string()],
            bind_addr: "127.0.0.2:49152".parse().unwrap(),
            lan: false,
            web_write: false,
            max_clients: 1,
            once: false,
            headless: true,
            token: "abc".to_string(),
            backspace: vec![0x7f],
            word_erase: vec![0x17],
        };

        let record = SessionRecord::from_config(&config);

        assert_eq!(record.local_url, "http://127.0.0.2:49152/t/abc".to_string());
    }
}
