//! A tool-less model server on the person's own machines, checked at every connection.
//!
//! Sources, read 2026-10-31: https://docs.ollama.com/api/openai-compatibility and
//! https://github.com/ollama/ollama/blob/main/openai/openai.go (nested JSON schema),
//! https://github.com/ggml-org/llama.cpp/blob/master/tools/server/server-common.cpp,
//! https://lmstudio.ai/docs/developer/openai-compat/structured-output (the same reply shape).
//! ureq 3.4.2's `unversioned/resolver.rs`, `agent.rs` and `config.rs`: a custom resolver pins
//! the checked addresses to the connection; proxies must be disabled explicitly.
use crate::{
    guard::Sealed,
    ledger::{Line, Session},
    provider::{Provider, Reach, Reply, Usage, check_brief},
    text,
};
use persona_builder::error::{Error, Result};
use persona_builder_ingest::private::{read_private_optional, remove_private, write_private};
use serde_json::{Value, json};
use std::{
    net::{IpAddr, Ipv4Addr},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use ureq::unversioned::{
    resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver},
    transport::{DefaultConnector, NextTimeout},
};

pub const AGENT_ID: &str = "self-hosted";
pub const AGENT_NAME: &str = "A model server on your own machines";
pub const REACH: Reach = Reach::OwnMachines;
/// Saved in the assistant's state folder, 0600, beside limit.json.
pub const FILE: &str = "self-hosted.json";
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const BODY_CAP: u64 = 16 * 1024 * 1024;
const MODELS_CAP: u64 = 1024 * 1024;

/// This computer or the person's own network: loopback, the private ranges and link-local.
pub fn own_network(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let bits = u32::from(ip);
            bits & 0xff00_0000 == 0x7f00_0000
                || bits & 0xff00_0000 == 0x0a00_0000
                || bits & 0xfff0_0000 == 0xac10_0000
                || bits & 0xffff_0000 == 0xc0a8_0000
                || bits & 0xffff_0000 == 0xa9fe_0000
        }
        IpAddr::V6(ip) => {
            let bits = u128::from(ip);
            if bits >> 32 == 0xffff {
                return own_network(IpAddr::V4(Ipv4Addr::from(bits as u32)));
            }
            bits == 1 || bits >> 121 == 0x7e || bits >> 118 == 0x3fa
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    url: String,
    model: String,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    url: String,
    model: String,
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

impl Server {
    pub fn new(url: &str, model: &str) -> Result<Server> {
        let bad_url = || invalid(text::BAD_SERVER_URL);
        if url.len() > 255
            || !url.is_ascii()
            || url
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err(bad_url());
        }
        let (scheme, authority) = url
            .strip_prefix("http://")
            .map(|a| ("http", a))
            .or_else(|| url.strip_prefix("https://").map(|a| ("https", a)))
            .ok_or_else(bad_url)?;
        let authority = authority.strip_suffix('/').unwrap_or(authority);
        let (host, port, ip) = if let Some(rest) = authority.strip_prefix('[') {
            let (host, suffix) = rest.split_once(']').ok_or_else(bad_url)?;
            let ip = host.parse::<std::net::Ipv6Addr>().map_err(|_| bad_url())?;
            let port = if suffix.is_empty() {
                None
            } else {
                Some(suffix.strip_prefix(':').ok_or_else(bad_url)?)
            };
            (format!("[{ip}]"), port, Some(IpAddr::V6(ip)))
        } else {
            let (host, port) = authority
                .split_once(':')
                .map_or((authority, None), |(h, p)| (h, Some(p)));
            if host.is_empty()
                || host.len() > 253
                || !host.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && !label.starts_with('-')
                        && !label.ends_with('-')
                        && label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err(bad_url());
            }
            let ip = if host.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
                Some(IpAddr::V4(host.parse::<Ipv4Addr>().map_err(|_| bad_url())?))
            } else {
                None
            };
            (host.to_ascii_lowercase(), port, ip)
        };
        if port.is_some_and(|port| {
            port.is_empty()
                || !port.bytes().all(|b| b.is_ascii_digit())
                || port.parse::<u16>().ok().filter(|p| *p > 0).is_none()
        }) {
            return Err(bad_url());
        }
        if ip.is_some_and(|ip| !own_network(ip)) {
            return Err(invalid(text::SERVER_NOT_OWN));
        }
        if model.is_empty()
            || model.len() > 128
            || !model.as_bytes()[0].is_ascii_alphanumeric()
            || !model.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':' | b'/' | b'@')
            })
        {
            return Err(invalid(text::BAD_SERVER_MODEL));
        }
        let port = port.map(|p| format!(":{p}")).unwrap_or_default();
        Ok(Server {
            url: format!("{scheme}://{host}{port}"),
            model: model.into(),
        })
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    /// Read without creating state; damaged settings name their file.
    pub fn load(home: &Path, slug: &str) -> Result<Option<Server>> {
        let path = home.join(".claude/state").join(slug).join(FILE);
        let damaged = || Error::Invalid(text::server_damaged(&path));
        let Some(bytes) = read_private_optional(home, slug, FILE).map_err(|_| damaged())? else {
            return Ok(None);
        };
        let saved: Saved = serde_json::from_slice(&bytes).map_err(|_| damaged())?;
        Self::new(&saved.url, &saved.model)
            .map(Some)
            .map_err(|_| damaged())
    }
    /// Atomically save as 0600. The caller holds the state lock.
    pub fn save(&self, home: &Path, slug: &str) -> Result<()> {
        let bytes = serde_json::to_vec(&Saved {
            url: self.url.clone(),
            model: self.model.clone(),
        })
        .expect("strings serialize");
        write_private(home, slug, FILE, &bytes)?;
        Ok(())
    }
    /// Remove the saved server. The caller holds the state lock.
    pub fn remove(home: &Path, slug: &str) -> Result<bool> {
        remove_private(home, slug, FILE)
    }
}

#[derive(Debug)]
struct NotOwnNetwork;
impl std::fmt::Display for NotOwnNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(text::SERVER_NOT_OWN)
    }
}
impl std::error::Error for NotOwnNetwork {}
#[derive(Debug)]
struct OwnNetworkOnly;
impl Resolver for OwnNetworkOnly {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> std::result::Result<ResolvedSocketAddrs, ureq::Error> {
        let addresses = DefaultResolver::default().resolve(uri, config, timeout)?;
        if addresses.iter().all(|address| own_network(address.ip())) {
            Ok(addresses)
        } else {
            Err(ureq::Error::Other(Box::new(NotOwnNetwork)))
        }
    }
}
fn agent(timeout: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .proxy(None)
        .timeout_global(Some(timeout))
        .user_agent(format!("persona-builder/{}", persona_builder::VERSION))
        .build();
    ureq::Agent::with_parts(config, DefaultConnector::default(), OwnNetworkOnly)
}
fn transport_error(error: ureq::Error, timeout: Duration) -> (&'static str, Error) {
    match error {
        ureq::Error::Other(ref inner) if inner.is::<NotOwnNetwork>() => {
            ("refused", invalid(text::SERVER_NOT_OWN))
        }
        ureq::Error::Timeout(_) => (
            "cut-off",
            Error::Invalid(text::server_cut_off(timeout.as_secs().div_ceil(60).max(1))),
        ),
        ureq::Error::BodyExceedsLimit(_) => ("unreadable", invalid(text::SERVER_FAILED)),
        _ => ("unreachable", invalid(text::SERVER_UNREACHABLE)),
    }
}

pub struct SelfHosted {
    server: Server,
    agent: ureq::Agent,
    timeout: Duration,
    stop: Option<Arc<AtomicBool>>,
}
impl SelfHosted {
    pub fn new(server: Server) -> SelfHosted {
        Self {
            server,
            agent: agent(DEFAULT_TIMEOUT),
            timeout: DEFAULT_TIMEOUT,
            stop: None,
        }
    }
    pub fn load(home: &Path, slug: &str) -> Result<Option<SelfHosted>> {
        Ok(Server::load(home, slug)?.map(Self::new))
    }
    pub fn with_timeout(self, timeout: Duration) -> SelfHosted {
        Self {
            timeout,
            agent: agent(timeout),
            ..self
        }
    }
    pub fn with_stop(self, stop: Arc<AtomicBool>) -> SelfHosted {
        Self {
            stop: Some(stop),
            ..self
        }
    }
    pub fn server(&self) -> &Server {
        &self.server
    }
    /// The server answers and lists the model, within 30 seconds at most.
    pub fn check(&self) -> Result<()> {
        let timeout = CHECK_TIMEOUT.min(self.timeout);
        let mut response = agent(timeout)
            .get(format!("{}/v1/models", self.server.url))
            .call()
            .map_err(|e| transport_error(e, timeout).1)?;
        if response.status() != 200 {
            return Err(invalid(text::SERVER_FAILED));
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MODELS_CAP)
            .read_to_vec()
            .map_err(|error| {
                if matches!(error, ureq::Error::BodyExceedsLimit(_)) {
                    invalid(text::SERVER_FAILED)
                } else {
                    transport_error(error, timeout).1
                }
            })?;
        let body: Value =
            serde_json::from_slice(&bytes).map_err(|_| invalid(text::SERVER_FAILED))?;
        let models = body
            .get("data")
            .and_then(Value::as_array)
            .filter(|models| models.iter().all(Value::is_object))
            .ok_or_else(|| invalid(text::SERVER_FAILED))?;
        if models.iter().any(|model| {
            model.get("id").and_then(Value::as_str) == Some(self.server.model.as_str())
        }) {
            Ok(())
        } else {
            Err(Error::Invalid(text::no_model(&self.server.model)))
        }
    }
    fn send(
        &self,
        brief: &crate::provider::Brief,
    ) -> std::result::Result<(Usage, Value), (&'static str, Error)> {
        let body = json!({
            "model": self.server.model,
            "messages": [
                {"role": "system", "content": brief.system},
                {"role": "user", "content": brief.prompt},
            ],
            "stream": false,
            "response_format": {
                "type": "json_schema",
                "json_schema": {"name": "reply", "strict": true, "schema": brief.schema},
            },
        });
        let mut response = self
            .agent
            .post(format!("{}/v1/chat/completions", self.server.url))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send(body.to_string())
            .map_err(|e| transport_error(e, self.timeout))?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(BODY_CAP)
            .read_to_vec()
            .map_err(|e| transport_error(e, self.timeout))?;
        if response.status() != 200 {
            return Err(("error", invalid(text::SERVER_FAILED)));
        }
        judge(&bytes)
    }
}
fn judge(bytes: &[u8]) -> std::result::Result<(Usage, Value), (&'static str, Error)> {
    let failed = |outcome| (outcome, invalid(text::SERVER_FAILED));
    let body: Value = serde_json::from_slice(bytes).map_err(|_| failed("unreadable"))?;
    let choice = &body["choices"][0];
    let message = &choice["message"];
    let content = message["content"]
        .as_str()
        .ok_or_else(|| failed("unreadable"))?;
    if message
        .get("tool_calls")
        .is_some_and(|calls| calls.as_array().is_none_or(|calls| !calls.is_empty()))
        || choice
            .get("finish_reason")
            .is_some_and(|reason| !reason.is_null() && reason.as_str() != Some("stop"))
    {
        return Err(failed("error"));
    }
    let output: Value = serde_json::from_str(content).map_err(|_| failed("no-output"))?;
    if !output.is_object() && !output.is_array() {
        return Err(failed("no-output"));
    }
    let usage = Usage {
        input_tokens: body["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
        output_tokens: body["usage"]["completion_tokens"].as_u64().unwrap_or(0),
        ..Usage::default()
    };
    Ok((usage, output))
}
impl Provider for SelfHosted {
    fn id(&self) -> &'static str {
        AGENT_ID
    }
    fn name(&self) -> &'static str {
        AGENT_NAME
    }
    fn reach(&self) -> Reach {
        REACH
    }
    fn model<'b>(&'b self, _asked: &'b str) -> &'b str {
        &self.server.model
    }
    fn call(&self, session: &Session, sealed: &Sealed) -> Result<Reply> {
        sealed.check(AGENT_ID, REACH)?;
        let brief = sealed.brief();
        check_brief(brief)?;
        if self
            .stop
            .as_ref()
            .is_some_and(|stop| stop.load(Ordering::SeqCst))
        {
            return Err(invalid(text::SERVER_STOPPED));
        }
        let started = session.start(AGENT_ID, brief.step, &self.server.model)?;
        let begun = Instant::now();
        let (outcome, usage, result) = match self.send(brief) {
            Ok((usage, output)) => (
                "ok",
                usage,
                Ok(Reply {
                    output,
                    usage,
                    cost_usd: 0.0,
                }),
            ),
            Err((outcome, error)) => (outcome, Usage::default(), Err(error)),
        };
        session.finish(
            started,
            &Line {
                agent: AGENT_ID,
                step: brief.step,
                model: &self.server.model,
                outcome,
                plan: None,
                seconds: begun.elapsed().as_secs_f64(),
                usage,
                cost_usd: 0.0,
                cost_from: "self-hosted",
            },
        )?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{guard::Guard, ledger::Cap, provider::Brief};
    use persona_builder::ceilings::Ceilings;
    use ureq::unversioned::transport::time::Duration as UreqDuration;

    #[test]
    fn resolver_returns_only_own_addresses_and_preserves_its_error_type() {
        let config = agent(DEFAULT_TIMEOUT);
        for (url, allowed) in [("http://127.0.0.1:9/", true), ("http://8.8.8.8:9/", false)] {
            let result = OwnNetworkOnly.resolve(
                &url.parse().unwrap(),
                config.config(),
                NextTimeout {
                    after: UreqDuration::NotHappening,
                    reason: ureq::Timeout::Global,
                },
            );
            if allowed {
                let addresses = result.unwrap();
                assert!(!addresses.is_empty());
                assert!(
                    addresses
                        .iter()
                        .all(|address| address.ip() == "127.0.0.1".parse::<IpAddr>().unwrap())
                );
            } else {
                assert!(
                    matches!(result, Err(ureq::Error::Other(ref error)) if error.is::<NotOwnNetwork>())
                );
            }
        }
        let error = config.get("http://8.8.8.8:9/").call().unwrap_err();
        assert!(matches!(error, ureq::Error::Other(ref error) if error.is::<NotOwnNetwork>()));
    }

    #[test]
    fn agent_disables_proxies_including_after_timeout_changes() {
        let server = SelfHosted::new(Server::new("http://localhost", "qwen").unwrap());
        assert!(server.agent.config().proxy().is_none());
        let server = server.with_timeout(Duration::from_secs(1));
        assert!(server.agent.config().proxy().is_none());
    }

    #[test]
    fn a_public_literal_bypassing_saved_validation_is_refused_and_recorded() {
        let home = std::env::temp_dir().join(format!(
            "persona-builder-public-literal-{}",
            std::process::id()
        ));
        std::fs::create_dir(&home).unwrap();
        let server = SelfHosted::new(Server {
            url: "http://8.8.8.8:9".into(),
            model: "qwen".into(),
        });
        let session = Session::open(&home, "wren", Cap::default()).unwrap();
        let schema = json!({"type":"object"});
        let brief = Brief {
            sources: &[persona_builder::ceilings::Source::Mail],
            step: "label",
            system: "Label records.",
            prompt: "records",
            model: "sonnet",
            schema: &schema,
        };
        let error = Guard::new(vec![&server], Ceilings::defaults())
            .call(&session, &brief)
            .unwrap_err();
        assert_eq!(error.to_string(), text::SERVER_NOT_OWN);
        let bytes = read_private_optional(&home, "wren", "runs.jsonl")
            .unwrap()
            .unwrap();
        let line: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(line["outcome"], "refused");
        assert_eq!(line["agent"], AGENT_ID);
        assert_eq!(line["model"], "qwen");
        assert_eq!(line["cost_usd"], 0.0);
        drop(session);
        std::fs::remove_dir_all(home).unwrap();
    }
}
