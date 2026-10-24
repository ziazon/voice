use crate::{secret::Secret, text};
use persona_builder::error::{Error, Result};
use serde_json::{Value, json};
use std::time::Duration;

pub enum Method {
    Get,
    Post,
}
pub struct HttpRequest<'a> {
    pub method: Method,
    pub url: String,
    /// Static values only, so a token cannot be put in a header by mistake.
    pub headers: Vec<(&'static str, &'static str)>,
    pub form: Vec<(&'static str, String)>,
    pub json: Option<serde_json::Value>,
    pub bearer: Option<&'a Secret>,
}
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
pub trait Http {
    fn send(&self, request: &HttpRequest<'_>) -> Result<HttpResponse>;
}
pub struct UreqHttp {
    agent: ureq::Agent,
}
impl Default for UreqHttp {
    fn default() -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(format!("persona-builder/{}", persona_builder::VERSION))
            .build();
        Self {
            agent: config.into(),
        }
    }
}
impl Http for UreqHttp {
    fn send(&self, request: &HttpRequest<'_>) -> Result<HttpResponse> {
        let mut builder = match request.method {
            Method::Get => self.agent.get(&request.url).force_send_body(),
            Method::Post => self.agent.post(&request.url),
        };
        for (name, value) in &request.headers {
            builder = builder.header(*name, *value);
        }
        if let Some(token) = request.bearer {
            let value = Secret::new(format!("Bearer {}", token.expose()));
            builder = builder.header("Authorization", value.expose());
        }
        let response = if !request.form.is_empty() {
            builder.send_form(
                request
                    .form
                    .iter()
                    .map(|(key, value)| (*key, value.as_str())),
            )
        } else if let Some(json) = &request.json {
            let body = serde_json::to_vec(json).map_err(|_| unreachable())?;
            builder
                .header("Content-Type", "application/json")
                .send(body)
        } else {
            builder.send_empty()
        };
        let mut response = response.map_err(|_| unreachable())?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                value
                    .to_str()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_owned()))
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| unreachable())?;
        let body = response
            .body_mut()
            .with_config()
            .limit(1_048_576)
            .read_to_vec()
            .map_err(|_| unreachable())?;
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}
pub struct Endpoints {
    login: String,
    api: String,
}
impl Endpoints {
    pub fn github() -> Self {
        Self {
            login: "https://github.com".into(),
            api: "https://api.github.com".into(),
        }
    }
    pub fn new(login: &str, api: &str) -> Result<Self> {
        Ok(Self {
            login: endpoint(login)?,
            api: endpoint(api)?,
        })
    }
}
pub const SCOPE: &str = "repo";
/// API version: https://docs.github.com/en/rest/about-the-rest-api/api-versions
pub const API_VERSION: &str = "2026-03-10";
/// Persona Builder's registered GitHub OAuth App (device flow on). None until it is registered.
const CLIENT_ID: Option<&str> = None;
pub fn client_id() -> Option<&'static str> {
    option_env!("PERSONA_BUILDER_GITHUB_CLIENT_ID").or(CLIENT_ID)
}
pub struct Github<'a> {
    http: &'a dyn Http,
    endpoints: Endpoints,
    client_id: String,
}
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
    device_code: Secret,
}
pub struct Account {
    pub login: String,
    pub id: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}
impl RepoRef {
    pub fn full(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}
impl<'a> Github<'a> {
    pub fn new(http: &'a dyn Http, endpoints: Endpoints, client_id: &str) -> Result<Self> {
        if !(1..=64).contains(&client_id.len())
            || !client_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.')
        {
            return Err(Error::Invalid(text::NOT_SET_UP.into()));
        }
        Ok(Self {
            http,
            endpoints,
            client_id: client_id.into(),
        })
    }
    /// Device flow: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps
    pub fn start_sign_in(&self) -> Result<DeviceCode> {
        let response = self.login(
            "/login/device/code",
            vec![
                ("client_id", self.client_id.clone()),
                ("scope", SCOPE.into()),
            ],
        )?;
        let body = sign_in_body(&response, &[200])?;
        let device_code = body["device_code"]
            .as_str()
            .filter(|s| (1..=256).contains(&s.chars().count()) && !s.chars().any(char::is_control))
            .ok_or_else(unexpected)?;
        let user_code = body["user_code"]
            .as_str()
            .filter(|s| {
                (1..=32).contains(&s.len())
                    && s.bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
            })
            .ok_or_else(unexpected)?;
        let verification_uri = body["verification_uri"]
            .as_str()
            .filter(|s| s.starts_with(&format!("{}/", self.endpoints.login)))
            .ok_or_else(unexpected)?;
        let interval = bounded(&body, "interval", 5, 60)?;
        let expires_in = bounded(&body, "expires_in", 900, 1800)?;
        Ok(DeviceCode {
            user_code: user_code.into(),
            verification_uri: verification_uri.into(),
            interval,
            expires_in,
            device_code: Secret::new(device_code.into()),
        })
    }
    /// Polling and backoff: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps
    pub fn finish_sign_in(
        &self,
        code: &DeviceCode,
        sleep: &mut dyn FnMut(Duration),
    ) -> Result<Secret> {
        let mut interval = code.interval;
        let mut elapsed = 0u64;
        loop {
            elapsed = elapsed
                .checked_add(interval)
                .filter(|total| *total <= code.expires_in)
                .ok_or_else(|| Error::Invalid(text::SIGN_IN_EXPIRED.into()))?;
            sleep(Duration::from_secs(interval));
            let response = self.login(
                "/login/oauth/access_token",
                vec![
                    ("client_id", self.client_id.clone()),
                    ("device_code", code.device_code.expose().into()),
                    (
                        "grant_type",
                        "urn:ietf:params:oauth:grant-type:device_code".into(),
                    ),
                ],
            )?;
            let body = sign_in_body(&response, &[200, 400])?;
            if let Some(token) = body.get("access_token") {
                let token = token
                    .as_str()
                    .filter(|s| {
                        (1..=255).contains(&s.len())
                            && s.bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"._~:+/=-".contains(&b))
                    })
                    .ok_or_else(unexpected)?;
                return Ok(Secret::new(token.into()));
            }
            let error = body["error"].as_str().ok_or_else(unexpected)?;
            match error {
                "authorization_pending" => {}
                "slow_down" => {
                    interval = interval
                        .saturating_add(5)
                        .max(body["interval"].as_u64().unwrap_or(0));
                }
                "expired_token" => return Err(Error::Invalid(text::SIGN_IN_EXPIRED.into())),
                "access_denied" => return Err(Error::Invalid(text::SIGN_IN_CANCELLED.into())),
                other
                    if (1..=40).contains(&other.len())
                        && other.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') =>
                {
                    return Err(Error::Invalid(text::sign_in_failed(other)));
                }
                _ => return Err(unexpected()),
            }
        }
    }
    /// Granted scopes: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps
    pub fn account(&self, token: &Secret) -> Result<Account> {
        let response = self.api(Method::Get, "/user", token, None)?;
        if response.status == 401 {
            return Err(Error::Invalid(text::SIGN_IN_FIRST.into()));
        }
        if response.status != 200 {
            return Err(unreachable());
        }
        if !response.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("x-oauth-scopes")
                && value.split(',').any(|scope| scope.trim() == SCOPE)
        }) {
            return Err(Error::Invalid(text::NO_REPO_PERMISSION.into()));
        }
        let body: Value = serde_json::from_slice(&response.body).map_err(|_| unreachable())?;
        let login = body["login"]
            .as_str()
            .filter(|s| valid_owner(s))
            .ok_or_else(unreachable)?;
        let id = body["id"]
            .as_u64()
            .filter(|id| *id > 0)
            .ok_or_else(unreachable)?;
        Ok(Account {
            login: login.into(),
            id,
        })
    }
    /// Private creation: https://docs.github.com/en/rest/repos/repos
    pub fn create_private_repo(&self, token: &Secret, name: &str) -> Result<RepoRef> {
        if !valid_repo_name(name) {
            return Err(Error::Invalid(text::BAD_REPO_NAME.into()));
        }
        let response = self.api(Method::Post, "/user/repos", token, Some(json!({
            "name": name, "private": true, "description": "My AI assistant, saved by Persona Builder",
            "auto_init": false, "has_issues": false, "has_projects": false, "has_wiki": false
        })))?;
        match response.status {
            201 => {}
            422 => return Err(Error::Invalid(text::repo_name_taken(name))),
            401 => return Err(Error::Invalid(text::SIGN_IN_FIRST.into())),
            403 => return Err(Error::Invalid(text::NO_REPO_PERMISSION.into())),
            _ => return Err(unreachable()),
        }
        let body: Value = serde_json::from_slice(&response.body).map_err(|_| unreachable())?;
        let owner = body["owner"]["login"]
            .as_str()
            .filter(|s| valid_owner(s))
            .ok_or_else(unreachable)?;
        let name = body["name"]
            .as_str()
            .filter(|s| valid_repo_name(s))
            .ok_or_else(unreachable)?;
        let repo = RepoRef {
            owner: owner.into(),
            name: name.into(),
        };
        self.confirm_private(token, &repo)?;
        Ok(repo)
    }
    /// Privacy readback: https://docs.github.com/en/rest/repos/repos
    pub fn confirm_private(&self, token: &Secret, repo: &RepoRef) -> Result<()> {
        if !valid_owner(&repo.owner) || !valid_repo_name(&repo.name) {
            return Err(Error::Invalid(text::NOT_ON_GITHUB.into()));
        }
        let response = self.api(Method::Get, &format!("/repos/{}", repo.full()), token, None)?;
        if response.status != 200 {
            return Err(Error::Invalid(text::repo_unconfirmed(&repo.full())));
        }
        let body: Value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
        if body["private"].as_bool() == Some(true) && body["visibility"].as_str() == Some("private")
        {
            Ok(())
        } else {
            Err(Error::Invalid(text::repo_public(&repo.full())))
        }
    }
}
pub fn valid_owner(owner: &str) -> bool {
    (1..=39).contains(&owner.len())
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
pub fn valid_repo_name(name: &str) -> bool {
    (1..=100).contains(&name.len())
        && name != "."
        && name != ".."
        && !name.ends_with(".git")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
pub fn suggested_repo_name(slug: &str) -> String {
    format!("{slug}-assistant")
}
pub fn github_repo_of(url: &str) -> Option<RepoRef> {
    let (host, path) = if let Some(rest) = url.strip_prefix("https://") {
        let (authority, path) = rest.split_once('/')?;
        let host = if let Some((userinfo, host)) = authority.split_once('@') {
            if userinfo.is_empty()
                || userinfo.chars().any(|c| {
                    c.is_control() || c.is_whitespace() || matches!(c, '/' | '?' | '#' | '\\')
                })
            {
                return None;
            }
            host
        } else {
            authority
        };
        (host, path)
    } else if let Some(rest) = url.strip_prefix("ssh://git@") {
        rest.split_once('/')?
    } else {
        let rest = url.strip_prefix("git@")?;
        rest.split_once(':')?
    };
    if !host.eq_ignore_ascii_case("github.com") {
        return None;
    }
    let path = path.strip_suffix('/').unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    if !valid_owner(owner) || !valid_repo_name(name) {
        return None;
    }
    Some(RepoRef {
        owner: owner.into(),
        name: name.into(),
    })
}

impl Github<'_> {
    fn login(&self, path: &str, form: Vec<(&'static str, String)>) -> Result<HttpResponse> {
        self.http.send(&HttpRequest {
            method: Method::Post,
            url: format!("{}{path}", self.endpoints.login),
            headers: vec![("Accept", "application/json")],
            form,
            json: None,
            bearer: None,
        })
    }
    fn api(
        &self,
        method: Method,
        path: &str,
        token: &Secret,
        json: Option<Value>,
    ) -> Result<HttpResponse> {
        self.http.send(&HttpRequest {
            method,
            url: format!("{}{path}", self.endpoints.api),
            headers: vec![
                ("Accept", "application/vnd.github+json"),
                ("X-GitHub-Api-Version", API_VERSION),
            ],
            form: vec![],
            json,
            bearer: Some(token),
        })
    }
}
fn unreachable() -> Error {
    Error::Invalid(text::GITHUB_UNREACHABLE.into())
}
fn unexpected() -> Error {
    Error::Invalid(text::sign_in_failed("unexpected reply"))
}
fn sign_in_body(response: &HttpResponse, statuses: &[u16]) -> Result<Value> {
    if !statuses.contains(&response.status) {
        return Err(unexpected());
    }
    let body: Value = serde_json::from_slice(&response.body).map_err(|_| unexpected())?;
    if !body.is_object() {
        return Err(unexpected());
    }
    Ok(body)
}
fn bounded(body: &Value, key: &str, default: u64, maximum: u64) -> Result<u64> {
    match body.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .filter(|n| (1..=maximum).contains(n))
            .ok_or_else(unexpected),
    }
}
fn endpoint(base: &str) -> Result<String> {
    let base = base.strip_suffix('/').unwrap_or(base);
    let valid = if let Some(host) = base.strip_prefix("https://") {
        !host.is_empty()
            && host.split('.').all(|label| {
                !label.is_empty()
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
    } else if let Some(port) = base.strip_prefix("http://127.0.0.1:") {
        !port.is_empty()
            && port.bytes().all(|b| b.is_ascii_digit())
            && port.parse::<u16>().is_ok_and(|p| p > 0)
    } else {
        false
    };
    if valid {
        Ok(base.into())
    } else {
        Err(Error::Invalid("not a GitHub address".into()))
    }
}
