use crate::session::RunConfig;

pub const SESSION_ID_ENV: &str = "RTERM_SESSION_ID";
pub const SESSION_NETWORK_ENV: &str = "RTERM_SESSION_NETWORK";
pub const SESSION_ACCESS_ENV: &str = "RTERM_SESSION_ACCESS";
pub const SESSION_CONTROL_ENV: &str = "RTERM_SESSION_CONTROL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMetadata {
    pub id: String,
    pub network: &'static str,
    pub access: &'static str,
    pub control: &'static str,
}

impl SessionMetadata {
    pub fn from_config(config: &RunConfig) -> Self {
        Self {
            id: std::process::id().to_string(),
            network: if config.bind_addr.ip().is_loopback() {
                "local"
            } else {
                "lan"
            },
            access: if config.web_write { "rw" } else { "ro" },
            control: if config.headless { "web" } else { "shared" },
        }
    }

    pub fn environment(&self) -> [(&'static str, &str); 4] {
        [
            (SESSION_ID_ENV, &self.id),
            (SESSION_NETWORK_ENV, self.network),
            (SESSION_ACCESS_ENV, self.access),
            (SESSION_CONTROL_ENV, self.control),
        ]
    }
}

pub fn segment() -> Option<String> {
    segment_from(|name| std::env::var(name).ok())
}

fn segment_from(mut get: impl FnMut(&str) -> Option<String>) -> Option<String> {
    let id = get(SESSION_ID_ENV)?;
    let network = get(SESSION_NETWORK_ENV)?;
    let access = get(SESSION_ACCESS_ENV)?;
    let control = get(SESSION_CONTROL_ENV)?;

    if id.is_empty()
        || id.len() > 10
        || !id.bytes().all(|byte| byte.is_ascii_digit())
        || !matches!(network.as_str(), "local" | "lan")
        || !matches!(access.as_str(), "ro" | "rw")
        || !matches!(control.as_str(), "shared" | "web")
    {
        return None;
    }

    Some(format!("rterm:{id} {network}/{access}/{control}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_session_metadata_formats_a_compact_segment() {
        let values = [
            ("RTERM_SESSION_ID", "4260"),
            ("RTERM_SESSION_NETWORK", "lan"),
            ("RTERM_SESSION_ACCESS", "rw"),
            ("RTERM_SESSION_CONTROL", "shared"),
        ];

        assert_eq!(
            segment_from(|name| {
                values
                    .iter()
                    .find(|(candidate, _)| *candidate == name)
                    .map(|(_, value)| (*value).to_string())
            }),
            Some("rterm:4260 lan/rw/shared".to_string())
        );
    }

    #[test]
    fn missing_or_invalid_metadata_hides_the_module() {
        assert_eq!(segment_from(|_| None), None);
        assert_eq!(
            segment_from(|name| (name == "RTERM_SESSION_ID").then(|| "4260".to_string())),
            None
        );
        assert_eq!(
            segment_from(|name| {
                Some(match name {
                    SESSION_ID_ENV => "\u{1b}[31m".to_string(),
                    SESSION_NETWORK_ENV => "lan".to_string(),
                    SESSION_ACCESS_ENV => "rw".to_string(),
                    SESSION_CONTROL_ENV => "shared".to_string(),
                    _ => return None,
                })
            }),
            None
        );
    }

    #[test]
    fn metadata_contains_only_safe_prompt_fields() {
        let config = RunConfig {
            command: vec!["pwsh".to_string()],
            bind_addr: "0.0.0.0:7843".parse().unwrap(),
            lan: true,
            web_write: true,
            max_clients: 1,
            once: false,
            headless: true,
            token: "do-not-export-this-token".to_string(),
            backspace: vec![0x7f],
            word_erase: vec![0x17],
        };

        let metadata = SessionMetadata::from_config(&config);
        let environment = metadata.environment();

        assert_eq!(metadata.network, "lan");
        assert_eq!(metadata.access, "rw");
        assert_eq!(metadata.control, "web");
        assert_eq!(environment.len(), 4);
        assert!(
            environment
                .iter()
                .all(|(name, value)| !name.contains("TOKEN")
                    && !value.contains(config.token.as_str()))
        );
    }
}
