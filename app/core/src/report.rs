//! Opt-in error and crash reports (off by default). Each report is one PostHog `$exception` event:
//! a category, a scrubbed message, the app and macOS versions and a random install id. No repo,
//! folder, pet or account names, no paths and no token ever leave in a report.
use std::time::Duration as StdDuration;

use chrono::Utc;
use reqwest::blocking::Client;
use uuid::Uuid;

use crate::github::NetEntry;

/// A report ready to send: `kind` is a fixed category (`save`, `load`, `crash`, …), `message` is scrubbed.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub kind: String,
    pub message: String,
    /// Crashes are unhandled; everything else was caught and shown to the user.
    pub handled: bool,
}

const MAX_MESSAGE: usize = 400;

/// Removes anything personal from `text`: every word holding a path, and every `private` string
/// (pet names, repo and account names). Then caps the length.
pub fn scrub(text: &str, private: &[&str]) -> String {
    let mut out: String = text.split(' ').map(|w| if w.contains('/') || w.contains('\\') { "<path>" } else { w }).collect::<Vec<_>>().join(" ");
    let mut private: Vec<&str> = private.iter().copied().filter(|p| p.len() >= 2).collect();
    // longest first, so "me/strays" goes before "strays"
    private.sort_by_key(|p| std::cmp::Reverse(p.len()));
    for p in private {
        out = out.replace(p, "<name>");
    }
    out.chars().take(MAX_MESSAGE).collect()
}

fn host_allowed(host: &str) -> bool {
    let https_posthog = host.strip_prefix("https://").is_some_and(|h| !h.contains('/') && h.ends_with(".posthog.com"));
    https_posthog || (cfg!(any(test, feature = "devtools")) && host.starts_with("http://127.0.0.1:"))
}

pub struct Reporter {
    http: Client,
    host: String,
    key: String,
}

impl Reporter {
    /// `host` must be a PostHog ingestion host (`https://us.i.posthog.com`, `https://eu.i.posthog.com`).
    pub fn new(host: &str, key: &str) -> Result<Self, String> {
        let host = host.trim_end_matches('/');
        if !host_allowed(host) {
            return Err(format!("Error reports can only go to PostHog, not {host}"));
        }
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(StdDuration::from_secs(10))
            .user_agent(concat!("Strays/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { http, host: host.into(), key: key.into() })
    }

    /// Sends one report. Returns the network-log entry either way, so every request shows in the activity log.
    pub fn send(&self, install: Uuid, version: &str, os: &str, r: &Report) -> (Result<(), String>, NetEntry) {
        let path = "/i/v0/e/";
        let body = serde_json::json!({
            "api_key": self.key,
            "event": "$exception",
            "distinct_id": install.to_string(),
            "properties": {
                "$exception_list": [{ "type": r.kind, "value": r.message, "mechanism": { "handled": r.handled, "synthetic": false } }],
                "$process_person_profile": false,
                "$lib": "strays",
                "app_version": version,
                "os_version": os,
            },
        });
        let res = self.http.post(format!("{}{path}", self.host)).json(&body).send();
        let status = res.as_ref().map_or(0, |r| r.status().as_u16());
        let entry = NetEntry {
            at: Utc::now(),
            method: "POST".into(),
            host: self.host.split("://").nth(1).unwrap_or(&self.host).into(),
            path: path.into(),
            status,
            reason: "error report".into(),
        };
        let result = match res {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(format!("PostHog answered {}", r.status())),
            Err(e) => Err(e.to_string()),
        };
        (result, entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    #[test]
    fn scrubbing_drops_paths_and_names() {
        let s = scrub("Couldn't save Miso: /Users/me/Library/x.json is read-only (me/strays, strays)", &["Miso", "me/strays", "strays", "x"]);
        assert_eq!(s, "Couldn't save <name>: <path> is read-only <path> <name>)");
        assert_eq!(scrub(&"a".repeat(1000), &[]).len(), MAX_MESSAGE);
    }

    #[test]
    fn only_posthog_hosts_are_allowed() {
        assert!(Reporter::new("https://us.i.posthog.com", "k").is_ok());
        assert!(Reporter::new("https://eu.i.posthog.com/", "k").is_ok());
        assert!(Reporter::new("https://evil.example", "k").is_err());
        assert!(Reporter::new("http://us.i.posthog.com", "k").is_err());
        assert!(Reporter::new("https://posthog.com.evil.example", "k").is_err());
    }

    #[test]
    fn a_report_is_one_exception_event_without_personal_data() {
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        let got = std::thread::spawn(move || {
            let (mut c, _) = server.accept().unwrap();
            let mut r = BufReader::new(c.try_clone().unwrap());
            let mut len = 0;
            let mut line = String::new();
            while r.read_line(&mut line).unwrap() > 2 {
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
                line.clear();
            }
            let mut body = vec![0; len];
            r.read_exact(&mut body).unwrap();
            c.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}").unwrap();
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()
        });
        let rep = Reporter::new(&format!("http://127.0.0.1:{port}"), "phc_test").unwrap();
        let id = Uuid::new_v4();
        let (ok, entry) = rep.send(id, "0.1.0", "26.5", &Report { kind: "save".into(), message: "Couldn't save <name>".into(), handled: true });
        assert_eq!(ok, Ok(()));
        assert_eq!((entry.method.as_str(), entry.path.as_str(), entry.status, entry.reason.as_str()), ("POST", "/i/v0/e/", 200, "error report"));
        let body = got.join().unwrap();
        assert_eq!(body["event"], "$exception");
        assert_eq!(body["distinct_id"], id.to_string());
        assert_eq!(body["properties"]["$exception_list"][0]["type"], "save");
        assert_eq!(body["properties"]["$process_person_profile"], false);
    }
}
