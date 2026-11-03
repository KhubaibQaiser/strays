//! GitHub client (spec §6): read-only repo metadata through a fine-grained token.
//! Only `api.github.com` is reachable (dev builds may also use a local fake server), redirects are
//! never followed, and the token is never logged or shown.
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use reqwest::StatusCode;
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, IF_NONE_MATCH};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::clock::Time;
use crate::pet::{ActivitySource, Pet, StateKind};
use crate::settings::Settings;

pub const API: &str = "https://api.github.com";
const AFFILIATION: &str = "affiliation=owner,collaborator,organization_member";

#[derive(Clone, Debug, PartialEq)]
pub enum GhError {
    /// Only fine-grained tokens (`github_pat_…`) are accepted.
    WrongTokenType,
    HostNotAllowed(String),
    /// 401: token revoked, expired or wrong. Pets freeze until it's replaced.
    Unauthorized,
    /// Primary or secondary rate limit. Wait at least this long before retrying.
    RateLimited { retry_after: Option<Duration>, reset_at: Option<Time> },
    Forbidden,
    Http(u16),
    Network(String),
    Parse(String),
}

impl std::fmt::Display for GhError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GhError::WrongTokenType => f.write_str("Only fine-grained tokens are accepted"),
            GhError::HostNotAllowed(h) => write!(f, "Strays only talks to api.github.com, not {h}"),
            GhError::Unauthorized => f.write_str("Token expired or revoked"),
            GhError::RateLimited { .. } => f.write_str("GitHub rate limit; waiting"),
            GhError::Forbidden => f.write_str("GitHub refused access (403)"),
            GhError::Http(code) if *code >= 500 => write!(f, "GitHub is having trouble ({code})"),
            GhError::Http(code) => write!(f, "GitHub answered with an error ({code})"),
            GhError::Network(_) => f.write_str("Offline"),
            GhError::Parse(_) => f.write_str("GitHub sent something Strays didn't understand"),
        }
    }
}

/// Response metadata worth keeping from every reply.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub token_expires: Option<Time>,
    pub rate_remaining: Option<u32>,
    pub rate_reset: Option<Time>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Owner {
    pub login: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Repo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: Owner,
    pub private: bool,
    pub fork: bool,
    pub archived: bool,
    pub created_at: Time,
    /// Null for a repo nobody has pushed to yet.
    pub pushed_at: Option<Time>,
}

/// The newest published release (§16 update check). Serialized into `sync-state.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
}

/// Where releases are published; update checks ask only this repo.
pub const RELEASES_REPO: &str = "KhubaibQaiser/strays";

/// True when `tag` (e.g. `v1.2.0`) is a later version than `current` (e.g. `1.1.3`). Unparseable tags never are.
pub fn is_newer(tag: &str, current: &str) -> bool {
    let parse = |v: &str| -> Option<Vec<u64>> { v.trim_start_matches('v').split('.').map(|p| p.parse().ok()).collect() };
    matches!((parse(tag), parse(current)), (Some(t), Some(c)) if t > c)
}

#[derive(Debug, PartialEq)]
pub struct Validation {
    pub login: String,
    pub repo_count: u64,
    pub expires: Option<Time>,
}

#[derive(Debug, PartialEq)]
pub enum Listing {
    /// 304: nothing pushed since the stored ETag. Doesn't count against the rate limit.
    NotModified,
    Changed { repos: Vec<Repo>, etag: Option<String>, complete: bool },
}

pub struct Github {
    http: Client,
    base: String,
    token: String,
    meta: RefCell<Meta>,
    reason: String,
    log: RefCell<Vec<NetEntry>>,
}

/// One line of the network activity log (§8.5). Paths name repos by id only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetEntry {
    pub at: Time,
    pub method: String,
    pub host: String,
    pub path: String,
    /// HTTP status, or 0 when the request never got an answer.
    pub status: u16,
    /// scheduled · manual · validation
    pub reason: String,
}

fn host_allowed(base: &str) -> bool {
    base == API || (cfg!(any(test, feature = "devtools")) && base.starts_with("http://127.0.0.1:"))
}

impl Github {
    pub fn new(token: &str, base: &str) -> Result<Self, GhError> {
        if !token.starts_with("github_pat_") {
            return Err(GhError::WrongTokenType);
        }
        if !host_allowed(base) {
            return Err(GhError::HostNotAllowed(base.into()));
        }
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(StdDuration::from_secs(30))
            .user_agent(concat!("Strays/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| GhError::Network(e.to_string()))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').into(),
            token: token.into(),
            meta: Default::default(),
            reason: "scheduled".into(),
            log: Default::default(),
        })
    }

    /// A client without a token, for public endpoints only (the update check): the token never leaves for them.
    pub fn anonymous(base: &str) -> Result<Self, GhError> {
        let mut gh = Self::new("github_pat_", base)?;
        gh.token.clear();
        Ok(gh)
    }

    /// Labels this client's requests in the network log.
    pub fn with_reason(mut self, reason: &str) -> Self {
        self.reason = reason.into();
        self
    }

    /// Requests made since the last call, for the network activity log.
    pub fn take_log(&self) -> Vec<NetEntry> {
        self.log.take()
    }

    fn record(&self, url: &str, status: u16) {
        let host = self.base.split("://").nth(1).unwrap_or(&self.base).to_string();
        let path = url.strip_prefix(&self.base).unwrap_or(url).to_string();
        self.log.borrow_mut().push(NetEntry { at: Utc::now(), method: "GET".into(), host, path, status, reason: self.reason.clone() });
    }

    pub fn meta(&self) -> Meta {
        self.meta.borrow().clone()
    }

    /// GET a path under the base, or a full URL (pagination links) that must also be under the base.
    fn get(&self, path_or_url: &str, etag: Option<&str>) -> Result<Response, GhError> {
        let url = if path_or_url.starts_with('/') { format!("{}{path_or_url}", self.base) } else { path_or_url.to_string() };
        if !url.starts_with(&format!("{}/", self.base)) {
            return Err(GhError::HostNotAllowed(url));
        }
        let mut req = self.http.get(&url).header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if !self.token.is_empty() {
            req = req.bearer_auth(&self.token);
        }
        if let Some(tag) = etag {
            req = req.header(IF_NONE_MATCH, tag);
        }
        let res = req.send().map_err(|e| {
            self.record(&url, 0);
            GhError::Network(e.without_url().to_string())
        })?;
        self.record(&url, res.status().as_u16());
        let meta = parse_meta(res.headers(), &self.meta.borrow());
        self.meta.replace(meta);
        let status = res.status();
        if status.is_success() || status == StatusCode::NOT_MODIFIED || status == StatusCode::NOT_FOUND {
            return Ok(res);
        }
        Err(status_error(status, res.headers()))
    }

    /// Setup check (§6.2): who the token belongs to, how many repos it sees, when it expires.
    pub fn validate(&self) -> Result<Validation, GhError> {
        let res = self.get("/user", None)?;
        if res.status() == StatusCode::NOT_FOUND {
            return Err(GhError::Http(404));
        }
        #[derive(Deserialize)]
        struct User {
            login: String,
        }
        let user: User = res.json().map_err(|e| GhError::Parse(e.to_string()))?;
        let res = self.get(&format!("/user/repos?per_page=1&{AFFILIATION}"), None)?;
        let last = link(res.headers(), "last").and_then(|u| query_param(&u, "page")).and_then(|p| p.parse().ok());
        let repo_count = match last {
            Some(n) => n,
            None => res.json::<Vec<serde_json::Value>>().map_err(|e| GhError::Parse(e.to_string()))?.len() as u64,
        };
        Ok(Validation { login: user.login, repo_count, expires: self.meta().token_expires })
    }

    /// Repos sorted by last push, newest first. With `since`, stops after the first page that reaches
    /// back before it; without, fetches every page (`complete: true`). The ETag only applies to page 1.
    pub fn list_repos(&self, since: Option<Time>, etag: Option<&str>) -> Result<Listing, GhError> {
        let mut next = Some(format!("/user/repos?sort=pushed&per_page=100&{AFFILIATION}"));
        let mut repos = Vec::new();
        let mut first_etag = None;
        let mut first = true;
        while let Some(url) = next.take() {
            let res = self.get(&url, if first { etag } else { None })?;
            if res.status() == StatusCode::NOT_MODIFIED {
                return Ok(Listing::NotModified);
            }
            if first {
                first_etag = res.headers().get("etag").and_then(|v| v.to_str().ok()).map(String::from);
                first = false;
            }
            next = link(res.headers(), "next");
            let page: Vec<Repo> = res.json().map_err(|e| GhError::Parse(e.to_string()))?;
            let reached_since = since.is_some_and(|s| page.iter().any(|r| r.pushed_at.is_none_or(|p| p < s)));
            repos.extend(page);
            if reached_since {
                return Ok(Listing::Changed { repos, etag: first_etag, complete: next.is_none() });
            }
        }
        Ok(Listing::Changed { repos, etag: first_etag, complete: true })
    }

    /// One repo by its stable id, so renames and transfers still resolve. `None` = gone (404).
    pub fn repo_by_id(&self, id: u64) -> Result<Option<Repo>, GhError> {
        let res = self.get(&format!("/repositories/{id}"), None)?;
        if res.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        res.json().map(Some).map_err(|e| GhError::Parse(e.to_string()))
    }

    /// The newest published release of `repo`. `None` = nothing published yet (404).
    pub fn latest_release(&self, repo: &str) -> Result<Option<Release>, GhError> {
        let res = self.get(&format!("/repos/{repo}/releases/latest"), None)?;
        if res.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        res.json().map(Some).map_err(|e| GhError::Parse(e.to_string()))
    }
}

fn status_error(status: StatusCode, h: &HeaderMap) -> GhError {
    let header = |name: &str| h.get(name).and_then(|v| v.to_str().ok());
    match status.as_u16() {
        401 => GhError::Unauthorized,
        403 | 429 => {
            let retry_after = header("retry-after").and_then(|v| v.parse().ok()).map(Duration::seconds);
            let exhausted = header("x-ratelimit-remaining") == Some("0");
            let reset_at = header("x-ratelimit-reset").and_then(|v| v.parse().ok()).and_then(|s| DateTime::from_timestamp(s, 0));
            if retry_after.is_some() || exhausted || status == StatusCode::TOO_MANY_REQUESTS {
                GhError::RateLimited { retry_after, reset_at: if exhausted { reset_at } else { None } }
            } else {
                GhError::Forbidden
            }
        }
        code => GhError::Http(code),
    }
}

fn parse_meta(h: &HeaderMap, prev: &Meta) -> Meta {
    let header = |name: &str| h.get(name).and_then(|v| v.to_str().ok());
    Meta {
        token_expires: header("github-authentication-token-expiration").and_then(parse_expiry).or(prev.token_expires),
        rate_remaining: header("x-ratelimit-remaining").and_then(|v| v.parse().ok()).or(prev.rate_remaining),
        rate_reset: header("x-ratelimit-reset")
            .and_then(|v| v.parse().ok())
            .and_then(|s| DateTime::from_timestamp(s, 0))
            .or(prev.rate_reset),
    }
}

/// e.g. "2026-11-29 13:28:15 UTC" (seen 2026-10-30) or "2026-11-29 13:28:15 +0000".
fn parse_expiry(v: &str) -> Option<Time> {
    if let Some(utc) = v.strip_suffix(" UTC") {
        return NaiveDateTime::parse_from_str(utc, "%Y-%m-%d %H:%M:%S").ok().map(|n| n.and_utc());
    }
    DateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S %z").ok().map(|d| d.with_timezone(&Utc))
}

/// The URL for `rel` in a `Link` header.
fn link(h: &HeaderMap, rel: &str) -> Option<String> {
    let value = h.get("link")?.to_str().ok()?;
    value.split(',').find_map(|part| {
        let (url, params) = part.split_once(';')?;
        params.contains(&format!("rel=\"{rel}\"")).then(|| url.trim().trim_start_matches('<').trim_end_matches('>').to_string())
    })
}

fn query_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|kv| kv.strip_prefix(&format!("{name}=")).map(String::from))
}

/// Retry delay after `failures` consecutive failed syncs: 1, 2, 4 … minutes, at most 1 h (§6.3).
pub fn backoff(failures: u32) -> Duration {
    Duration::minutes(1i64 << failures.saturating_sub(1).min(6)).min(Duration::hours(1))
}

/// Sync bookkeeping, saved as `sync-state.json`. No secrets.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncState {
    pub installed_at: Option<Time>,
    pub etag: Option<String>,
    /// Frozen while syncs fail, so moods can't get worse (§5.3).
    pub last_successful_check: Option<Time>,
    pub last_attempt: Option<Time>,
    pub failures: u32,
    pub disconnected: bool,
    pub token_expires: Option<Time>,
    pub rate_remaining: Option<u32>,
    pub rate_reset: Option<Time>,
    pub known_repo_ids: BTreeSet<u64>,
    /// Last update check (weekly while the opt-in setting is on) and a newer release it found.
    pub update_checked: Option<Time>,
    pub update: Option<Release>,
}

#[derive(Debug, Default, PartialEq)]
pub struct SyncReport {
    pub not_modified: bool,
    pub updated: Vec<Uuid>,
    pub renamed: Vec<Uuid>,
    pub graduated: Vec<Uuid>,
    /// Pets whose repo GitHub no longer finds (see `Pet::missing_since`).
    pub missing: Vec<Uuid>,
    /// Repos to offer as new projects (§5.9).
    pub new_projects: Vec<Repo>,
}

/// The network half of a sync, done without holding app state: the listing, plus a lookup by id for
/// each tracked repo missing from a complete listing (404 → `None`: deleted, or no longer visible to the token).
#[derive(Debug, PartialEq)]
pub struct Fetched {
    pub listing: Listing,
    pub by_id: Vec<(u64, Option<Repo>)>,
}

/// `full` lists every repo, ignoring the ETag, and looks up `tracked` repos missing from it, to catch
/// deletions. Use it on launch and about daily; otherwise the cheap incremental form.
pub fn fetch(gh: &Github, st: &SyncState, tracked: &[u64], full: bool) -> Result<Fetched, GhError> {
    let since = if full { None } else { st.last_successful_check };
    let listing = gh.list_repos(since, if full { None } else { st.etag.as_deref() })?;
    let mut by_id = Vec::new();
    if let Listing::Changed { repos, complete: true, .. } = &listing
        && full
    {
        let listed: BTreeSet<u64> = repos.iter().map(|r| r.id).collect();
        for &id in tracked.iter().filter(|id| !listed.contains(id)) {
            by_id.push((id, gh.repo_by_id(id)?));
        }
    }
    Ok(Fetched { listing, by_id })
}

/// The state half: applies a fetch result (or its error) to the sync state and pets.
/// Errors leave `last_successful_check` alone, so moods freeze (§5.3).
pub fn apply(result: Result<Fetched, GhError>, meta: &Meta, st: &mut SyncState, pets: &mut [Pet], settings: &Settings, now: Time) -> Result<SyncReport, GhError> {
    st.last_attempt = Some(now);
    st.installed_at.get_or_insert(now);
    st.token_expires = meta.token_expires.or(st.token_expires);
    st.rate_remaining = meta.rate_remaining;
    st.rate_reset = meta.rate_reset;
    let fetched = match result {
        Ok(f) => f,
        Err(e) => {
            st.failures += 1;
            // only a 401 disconnects and only a success reconnects; a network blip says nothing about the token
            st.disconnected |= e == GhError::Unauthorized;
            return Err(e);
        }
    };
    st.last_successful_check = Some(now);
    st.failures = 0;
    st.disconnected = false;
    let mut report = SyncReport::default();
    match fetched.listing {
        Listing::NotModified => report.not_modified = true,
        Listing::Changed { repos, etag, .. } => {
            st.etag = etag;
            apply_repos(pets, &repos, st, settings, now, &mut report);
        }
    }
    for (id, repo) in fetched.by_id {
        for pet in pets.iter_mut().filter(|p| p.project.github.as_ref().is_some_and(|g| g.id == id)) {
            match &repo {
                None => {
                    pet.missing_since.get_or_insert(now);
                    report.missing.push(pet.id);
                }
                Some(r) => apply_repos(std::slice::from_mut(pet), std::slice::from_ref(r), st, settings, now, &mut report),
            }
        }
    }
    Ok(report)
}

/// Fetch and apply in one go (tests, and callers that own the state outright).
pub fn sync(gh: &Github, st: &mut SyncState, pets: &mut [Pet], settings: &Settings, full: bool, now: Time) -> Result<SyncReport, GhError> {
    let tracked = tracked_ids(pets);
    let fetched = fetch(gh, st, &tracked, full);
    apply(fetched, &gh.meta(), st, pets, settings, now)
}

/// Repo ids of pets that still follow their repo (not graduated or let go).
pub fn tracked_ids(pets: &[Pet]) -> Vec<u64> {
    pets.iter()
        .filter(|p| !matches!(p.state.kind, StateKind::Graduated | StateKind::LetGo))
        .filter_map(|p| p.project.github.as_ref().map(|g| g.id))
        .collect()
}

fn apply_repos(pets: &mut [Pet], repos: &[Repo], st: &mut SyncState, settings: &Settings, now: Time, report: &mut SyncReport) {
    let installed_at = st.installed_at.unwrap_or(now);
    for repo in repos {
        let first_seen = st.known_repo_ids.insert(repo.id);
        let mut tracked = false;
        for pet in pets.iter_mut() {
            let Some(gh) = pet.project.github.as_mut().filter(|g| g.id == repo.id) else { continue };
            tracked = true;
            // found again: it wasn't deleted after all (a token with access again, or a repo made visible)
            pet.missing_since = None;
            if gh.full_name != repo.full_name {
                gh.full_name = repo.full_name.clone();
                gh.account = repo.owner.login.clone();
                report.renamed.push(pet.id);
            }
            // a finished pet doesn't eat; its repo may have a new pet now
            let gone = matches!(pet.state.kind, StateKind::Graduated | StateKind::LetGo);
            if let Some(pushed) = repo.pushed_at.filter(|&p| p > pet.activity.last && !gone) {
                pet.record_activity(pushed, ActivitySource::Github);
                report.updated.push(pet.id);
            }
            if repo.archived && pet.graduate(now, "Archived on GitHub").is_ok() {
                report.graduated.push(pet.id);
            }
        }
        let candidate = first_seen && !tracked && repo.created_at > installed_at && !repo.fork && !repo.archived && !settings.ignored(&repo.name);
        if candidate {
            report.new_projects.push(repo.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mood::Pace;
    use crate::pet::{Activity, GithubRef, Project, Species};
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    const TOKEN: &str = "github_pat_fake";

    struct Fake {
        child: Child,
        base: String,
    }

    impl Fake {
        fn start() -> Self {
            let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/fake-github/server.py");
            let mut child = Command::new("python3").arg(script).stdout(Stdio::piped()).spawn().expect("python3");
            let mut line = String::new();
            BufReader::new(child.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
            let port = line.trim().strip_prefix("PORT ").expect("PORT line");
            Self { child, base: format!("http://127.0.0.1:{port}") }
        }
        fn scenario(&self, name: &str) {
            Client::new().post(format!("{}/_scenario", self.base)).body(format!(r#"{{"name":"{name}"}}"#)).send().unwrap();
        }
        fn requests(&self) -> Vec<String> {
            Client::new().get(format!("{}/_requests", self.base)).send().unwrap().json().unwrap()
        }
        fn client(&self) -> Github {
            Github::new(TOKEN, &self.base).unwrap()
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = self.child.kill();
        }
    }

    fn t(s: &str) -> Time {
        s.parse().unwrap()
    }

    fn pet_for(id: u64, last: Time) -> Pet {
        let gh = GithubRef { id, full_name: format!("me/repo-{id}"), account: "me".into() };
        let mut p = Pet::new(last, "p", Species::Cat, Project { github: Some(gh), local: None }, Activity { last, source: ActivitySource::Github }, Pace::Normal);
        p.adopt(last).unwrap();
        p
    }

    #[test]
    fn rejects_classic_tokens_and_other_hosts() {
        assert_eq!(Github::new("ghp_abc", API).err(), Some(GhError::WrongTokenType));
        assert_eq!(Github::new("github_pat_x", "https://evil.example").err(), Some(GhError::HostNotAllowed("https://evil.example".into())));
        assert!(Github::new("github_pat_x", API).is_ok());
    }

    #[test]
    fn header_parsing() {
        assert_eq!(parse_expiry("2026-11-29 13:28:15 UTC"), Some(t("2026-11-29T13:28:15Z")));
        assert_eq!(parse_expiry("2026-11-29 18:28:15 +0500"), Some(t("2026-11-29T13:28:15Z")));
        assert_eq!(parse_expiry("soon"), None);
        let mut h = HeaderMap::new();
        h.insert("link", r#"<https://api.github.com/user/repos?page=2>; rel="next", <https://api.github.com/user/repos?page=9>; rel="last""#.parse().unwrap());
        assert_eq!(link(&h, "last").as_deref(), Some("https://api.github.com/user/repos?page=9"));
        assert_eq!(query_param(&link(&h, "next").unwrap(), "page").as_deref(), Some("2"));
        assert_eq!(link(&h, "prev"), None);
    }

    #[test]
    fn backoff_doubles_to_an_hour() {
        assert_eq!(backoff(1), Duration::minutes(1));
        assert_eq!(backoff(3), Duration::minutes(4));
        assert_eq!(backoff(7), Duration::minutes(64).min(Duration::hours(1)));
        assert_eq!(backoff(50), Duration::hours(1));
    }

    #[test]
    fn fake_validate_and_list() {
        let fake = Fake::start();
        let gh = fake.client();
        let v = gh.validate().unwrap();
        assert_eq!(v, Validation { login: "me".into(), repo_count: 250, expires: Some(t("2026-11-29T13:28:15Z")) });
        let log = gh.take_log();
        assert_eq!(log.iter().map(|e| (e.path.as_str(), e.status)).collect::<Vec<_>>()[0], ("/user", 200));
        assert!(log.iter().all(|e| e.host.starts_with("127.0.0.1:") && e.reason == "scheduled"));
        assert!(gh.take_log().is_empty(), "taking the log empties it");

        let Listing::Changed { repos, etag, complete } = gh.list_repos(None, None).unwrap() else { panic!() };
        assert_eq!((repos.len(), complete), (250, true));
        assert_eq!(etag.as_deref(), Some("\"fake-etag-1\""));
        assert_eq!(gh.list_repos(None, etag.as_deref()).unwrap(), Listing::NotModified);

        // incremental: repo i was pushed i*6 h before 2026-10-30 12:00; page 1 reaches back 25 days
        let Listing::Changed { repos, complete, .. } = gh.list_repos(Some(t("2026-10-20T00:00:00Z")), None).unwrap() else { panic!() };
        assert_eq!((repos.len(), complete), (100, false));
        let pages = fake.requests().iter().filter(|r| r.starts_with("/user/repos?sort")).count();
        assert_eq!(pages, 3 + 1 + 1, "full listing 3 pages, 304, then one incremental page");
    }

    #[test]
    fn fake_error_scenarios() {
        let fake = Fake::start();
        let gh = fake.client();
        fake.scenario("unauthorized");
        assert_eq!(gh.list_repos(None, None), Err(GhError::Unauthorized));
        fake.scenario("rate_limit");
        assert!(matches!(gh.list_repos(None, None), Err(GhError::RateLimited { retry_after: None, reset_at: Some(_) })));
        fake.scenario("secondary");
        assert_eq!(gh.list_repos(None, None), Err(GhError::RateLimited { retry_after: Some(Duration::seconds(60)), reset_at: None }));
        fake.scenario("too_many");
        assert_eq!(gh.list_repos(None, None), Err(GhError::RateLimited { retry_after: Some(Duration::seconds(30)), reset_at: None }));
        let wrong = Github::new("github_pat_wrong", &fake.base).unwrap();
        fake.scenario("ok");
        assert_eq!(wrong.validate(), Err(GhError::Unauthorized));
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("v0.10.0", "0.9.0"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("v0.1.0", "0.2.0"));
        assert!(!is_newer("v1.0.0-beta", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }

    #[test]
    fn fake_update_check_is_anonymous_and_logged() {
        let fake = Fake::start();
        let gh = Github::anonymous(&fake.base).unwrap().with_reason("update check");
        // the fake answers 400 if a token is sent
        let r = gh.latest_release(RELEASES_REPO).unwrap().unwrap();
        assert_eq!(r.tag_name, "v9.9.9");
        assert!(r.html_url.starts_with("https://github.com/KhubaibQaiser/strays/releases/"));
        let log = gh.take_log();
        assert_eq!(log.len(), 1);
        assert_eq!((log[0].path.as_str(), log[0].status, log[0].reason.as_str()), ("/repos/KhubaibQaiser/strays/releases/latest", 200, "update check"));
        fake.scenario("no_release");
        assert_eq!(gh.latest_release(RELEASES_REPO).unwrap(), None);
        // the anonymous client is still limited to the allowed host
        assert!(Github::anonymous("https://evil.example").is_err());
    }

    #[test]
    fn fake_network_error() {
        let gh = Github::new(TOKEN, "http://127.0.0.1:9").unwrap();
        assert!(matches!(gh.list_repos(None, None), Err(GhError::Network(_))));
    }

    #[test]
    fn fake_sync_updates_pets_and_freezes_on_failure() {
        let fake = Fake::start();
        let gh = fake.client();
        let settings = Settings::default();
        let mut st = SyncState { installed_at: Some(t("2025-01-01T00:00:00Z")), ..Default::default() };
        // 1 normal, 2 deleted, 3 archived (gone from the list too), 4 renamed; 5 listed and pushed
        let mut pets = vec![
            pet_for(5, t("2026-10-01T00:00:00Z")),
            pet_for(1000, t("2026-10-01T00:00:00Z")), // deleted: not listed, 404 by id
            pet_for(4, t("2026-10-01T00:00:00Z")),
        ];
        let t1 = t("2026-10-30T12:05:00Z");
        let r = sync(&gh, &mut st, &mut pets, &settings, true, t1).unwrap();
        assert_eq!(pets[0].activity.last, t("2026-10-29T06:00:00Z"));
        assert!(r.updated.contains(&pets[0].id));
        // a 404 can't tell deleted from hidden, so the pet asks rather than leaving
        assert_eq!(pets[1].state.kind, StateKind::Active);
        assert_eq!(pets[1].missing_since, Some(t1));
        assert_eq!(r.missing, vec![pets[1].id]);
        assert_eq!(st.last_successful_check, Some(t1));
        assert_eq!(st.known_repo_ids.len(), 250);
        // every fake repo was created after install
        assert_eq!(r.new_projects.len(), 250 - 25 - 2, "minus 25 forks and the 2 tracked repos");

        // failure: last_successful_check stays, failures count, 401 disconnects
        fake.scenario("unauthorized");
        let t2 = t("2026-10-30T13:00:00Z");
        assert_eq!(sync(&gh, &mut st, &mut pets, &settings, false, t2), Err(GhError::Unauthorized));
        assert_eq!((st.last_successful_check, st.failures, st.disconnected), (Some(t1), 1, true));
        assert_eq!(st.token_expires, Some(t("2026-11-29T13:28:15Z")));

        fake.scenario("ok");
        let r = sync(&gh, &mut st, &mut pets, &settings, false, t("2026-10-30T14:00:00Z")).unwrap();
        assert!(r.not_modified, "stored ETag from the full sync gives a 304");
        assert_eq!((st.failures, st.disconnected), (0, false));
    }

    #[test]
    fn fake_repo_by_id_handles_rename_archive_delete() {
        let fake = Fake::start();
        let gh = fake.client();
        assert_eq!(gh.repo_by_id(2).unwrap(), None);
        assert!(gh.repo_by_id(3).unwrap().unwrap().archived);
        let mut pets = vec![pet_for(4, t("2026-10-01T00:00:00Z")), pet_for(3, t("2026-10-01T00:00:00Z"))];
        let fetched = Fetched { listing: Listing::NotModified, by_id: vec![(4, gh.repo_by_id(4).unwrap()), (3, gh.repo_by_id(3).unwrap())] };
        let mut st = SyncState::default();
        let report = apply(Ok(fetched), &gh.meta(), &mut st, &mut pets, &Settings::default(), t("2026-10-30T00:00:00Z")).unwrap();
        assert_eq!(pets[0].project.github.as_ref().unwrap().full_name, "me/renamed");
        assert_eq!(pets[1].state.kind, StateKind::Graduated);
        assert_eq!((report.renamed.len(), report.graduated.len()), (1, 1));
        assert_eq!(tracked_ids(&pets), vec![4]);
    }

    #[test]
    fn a_missing_repo_confuses_its_pet_freezes_its_mood_and_clears_when_found() {
        let fake = Fake::start();
        let gh = fake.client();
        let settings = Settings::default();
        let mut pets = vec![pet_for(4, t("2026-10-01T00:00:00Z"))];
        let mut st = SyncState::default();
        let lost = t("2026-10-05T00:00:00Z");
        let fetched = Fetched { listing: Listing::NotModified, by_id: vec![(4, None)] };
        let r = apply(Ok(fetched), &gh.meta(), &mut st, &mut pets, &settings, lost).unwrap();
        assert_eq!((r.missing.clone(), pets[0].missing_since, pets[0].state.kind), (vec![pets[0].id], Some(lost), StateKind::Active));
        // weeks later the mood is still the one it had when the repo went missing
        let later = t("2026-11-05T00:00:00Z");
        assert_eq!(pets[0].idle_days(later, Some(later), &settings, &Utc), pets[0].idle_days(lost, Some(lost), &settings, &Utc));
        // a second 404 keeps the first date
        let fetched = Fetched { listing: Listing::NotModified, by_id: vec![(4, None)] };
        apply(Ok(fetched), &gh.meta(), &mut st, &mut pets, &settings, later).unwrap();
        assert_eq!(pets[0].missing_since, Some(lost));
        // visible again (say, a token with access): back to normal
        let fetched = Fetched { listing: Listing::NotModified, by_id: vec![(4, gh.repo_by_id(4).unwrap())] };
        apply(Ok(fetched), &gh.meta(), &mut st, &mut pets, &settings, later).unwrap();
        assert_eq!(pets[0].missing_since, None);
    }

    /// Live check with the user's test token: `cargo test -- --ignored live`. Prints no secrets.
    #[test]
    #[ignore]
    fn live_token() {
        let path = std::env::var("HOME").unwrap() + "/.config/strays/dev-token";
        let token = std::fs::read_to_string(path).unwrap();
        let gh = Github::new(token.trim(), API).unwrap();
        let v = gh.validate().unwrap();
        println!("login={} repos={} expires={:?}", v.login, v.repo_count, v.expires);
        assert!(v.expires.is_some());
        let Listing::Changed { repos, etag, .. } = gh.list_repos(None, None).unwrap() else { panic!() };
        assert_eq!(repos.len() as u64, v.repo_count);
        assert!(repos.windows(2).all(|w| w[0].pushed_at >= w[1].pushed_at), "sorted by push, newest first");
        assert_eq!(gh.list_repos(None, etag.as_deref()).unwrap(), Listing::NotModified);
        assert_eq!(gh.repo_by_id(repos[0].id).unwrap().map(|r| r.id), Some(repos[0].id));
        println!("listed={} first_pushed={:?} etag_304=ok", repos.len(), repos[0].pushed_at);
    }

    #[test]
    fn a_network_error_after_a_401_stays_disconnected() {
        let mut st = SyncState { disconnected: true, ..Default::default() };
        let now = t("2026-10-30T13:00:00Z");
        let r = apply(Err(GhError::Network("offline".into())), &Meta::default(), &mut st, &mut [], &Settings::default(), now);
        assert!(r.is_err());
        assert!(st.disconnected);
    }
}
