//! Local git as an activity source (spec §5.4), read with gix: no `git` binary, no fetch, no credentials.
use std::collections::HashMap;
use std::path::Path;

use chrono::DateTime;
use gix::bstr::ByteSlice;

use crate::clock::Time;
use crate::github::Repo;

#[derive(Debug, Default, PartialEq)]
pub struct LocalInfo {
    /// Latest committer date on any local branch.
    pub last_commit: Option<Time>,
    /// Latest committer date under `refs/remotes/*` (other devices' work, once the user has fetched).
    pub last_remote_ref: Option<Time>,
    /// `owner/name` (lowercase) of every remote that points at GitHub.
    pub github_slugs: Vec<String>,
}

/// Reads a working copy. `ssh_aliases` maps `Host` aliases to `HostName` from `~/.ssh/config`;
/// pass an empty map when that file isn't readable (the App Sandbox blocks it without a grant).
pub fn read(path: &Path, ssh_aliases: &HashMap<String, String>) -> Result<LocalInfo, String> {
    let repo = gix::open(path).map_err(|e| e.to_string())?;
    let mut info = LocalInfo::default();
    let refs = repo.references().map_err(|e| e.to_string())?;
    let newest = |iter: gix::reference::iter::Iter<'_, '_>| -> Option<Time> {
        iter.filter_map(Result::ok)
            .filter_map(|mut r| r.peel_to_commit().ok())
            .filter_map(|c| c.committer().ok().and_then(|s| s.time().ok()).map(|t| t.seconds))
            .max()
            .and_then(|s| DateTime::from_timestamp(s, 0))
    };
    info.last_commit = newest(refs.local_branches().map_err(|e| e.to_string())?);
    info.last_remote_ref = newest(refs.remote_branches().map_err(|e| e.to_string())?);
    for name in repo.remote_names() {
        let Ok(remote) = repo.find_remote(name.as_bstr()) else { continue };
        // find_remote applies the user's url.<base>.insteadOf rewrites
        if let Some(slug) = remote.url(gix::remote::Direction::Fetch).and_then(|u| github_slug(u, ssh_aliases))
            && !info.github_slugs.contains(&slug)
        {
            info.github_slugs.push(slug);
        }
    }
    Ok(info)
}

/// What `read` depends on, cheaply: modification times of HEAD, config, packed-refs and every folder
/// under `refs/` (git writes a ref by renaming a lock file into its folder, which bumps that folder's time).
/// Equal fingerprints mean `read` would return the same answer. `None` (a linked worktree, whose `.git`
/// is a file) means "read it every time".
pub fn fingerprint(path: &Path) -> Option<Vec<(String, std::time::SystemTime)>> {
    let git = path.join(".git");
    if !git.is_dir() {
        return None;
    }
    let mut out = Vec::new();
    let mut stamp = |p: &Path| {
        if let Ok(t) = std::fs::metadata(p).and_then(|m| m.modified()) {
            out.push((p.strip_prefix(&git).unwrap_or(p).display().to_string(), t));
        }
    };
    for f in ["HEAD", "config", "packed-refs"] {
        stamp(&git.join(f));
    }
    let mut dirs = vec![git.join("refs")];
    while let Some(d) = dirs.pop() {
        stamp(&d);
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        dirs.extend(entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()));
    }
    out.sort();
    Some(out)
}

/// `owner/name` for a GitHub remote URL in any form git accepts (https, ssh://, scp-style `git@host:o/n`).
pub fn github_slug(url: &gix::Url, ssh_aliases: &HashMap<String, String>) -> Option<String> {
    let host = url.host()?.to_lowercase();
    let host = ssh_aliases.get(&host).map(|h| h.to_lowercase()).unwrap_or(host);
    if host != "github.com" && host != "ssh.github.com" && host != "www.github.com" {
        return None;
    }
    let path = url.path.to_string();
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    (!owner.is_empty() && !name.is_empty() && !name.contains('/')).then(|| format!("{owner}/{name}").to_lowercase())
}

/// `Host` alias → `HostName` pairs from an ssh config. Wildcard patterns are skipped.
pub fn parse_ssh_config(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut hosts: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let (key, value) = match line.split_once(|c: char| c.is_whitespace() || c == '=') {
            Some((k, v)) => (k.to_lowercase(), v.trim_start_matches(|c: char| c.is_whitespace() || c == '=').trim()),
            None => continue,
        };
        match key.as_str() {
            "host" => hosts = value.split_whitespace().filter(|h| !h.contains(['*', '?', '!'])).map(str::to_lowercase).collect(),
            "match" => hosts.clear(),
            "hostname" => {
                for h in &hosts {
                    out.entry(h.clone()).or_insert_with(|| value.to_string());
                }
            }
            _ => {}
        }
    }
    out
}

#[derive(Debug, PartialEq)]
pub enum Match {
    Unique(u64),
    /// Several remotes point at different GitHub repos: ask the user (§5.4).
    Ambiguous(Vec<u64>),
    /// No GitHub remote the token can see: ask the user, or keep it local-only.
    None,
}

pub fn match_repo(info: &LocalInfo, repos: &[Repo]) -> Match {
    let mut ids: Vec<u64> = repos
        .iter()
        .filter(|r| info.github_slugs.iter().any(|s| s.eq_ignore_ascii_case(&r.full_name)))
        .map(|r| r.id)
        .collect();
    ids.dedup();
    match ids.len() {
        0 => Match::None,
        1 => Match::Unique(ids[0]),
        _ => Match::Ambiguous(ids),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::Owner;
    use std::process::Command;

    fn slug(url: &str, aliases: &HashMap<String, String>) -> Option<String> {
        github_slug(&gix::url::parse(url.as_bytes().as_bstr()).unwrap(), aliases)
    }

    #[test]
    fn url_forms() {
        let none = HashMap::new();
        assert_eq!(slug("https://github.com/Me/App.git", &none).as_deref(), Some("me/app"));
        assert_eq!(slug("https://github.com/me/app", &none).as_deref(), Some("me/app"));
        assert_eq!(slug("git@github.com:me/app.git", &none).as_deref(), Some("me/app"));
        assert_eq!(slug("ssh://git@ssh.github.com:443/me/app.git", &none).as_deref(), Some("me/app"));
        assert_eq!(slug("https://gitlab.com/me/app.git", &none), None);
        assert_eq!(slug("https://github.com/me", &none), None);
        let aliases = parse_ssh_config("Host gh work-gh\n  HostName github.com\n  User git\nHost *\n  HostName nope\n");
        assert_eq!(slug("gh:me/app.git", &aliases).as_deref(), Some("me/app"));
        assert_eq!(slug("git@work-gh:org/tool.git", &aliases).as_deref(), Some("org/tool"));
        assert_eq!(slug("git@other:org/tool.git", &aliases), None);
    }

    #[test]
    fn ssh_config_parsing() {
        let c = parse_ssh_config("# c\nHost=a\n\tHostName = github.com\nHost b\nHostName x.com\nHostName y.com\nMatch host z\nHostName q\n");
        assert_eq!(c.get("a").map(String::as_str), Some("github.com"));
        assert_eq!(c.get("b").map(String::as_str), Some("x.com"), "first value wins, like ssh");
        assert_eq!(c.len(), 2);
    }

    fn git(dir: &Path, args: &[&str], date: &str) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    #[test]
    fn reads_branches_remote_refs_and_remotes() {
        // test fixture only: the app itself never runs git
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let date1 = "2026-10-01T10:00:00Z";
        git(d, &["init", "-q", "-b", "main"], date1);
        git(d, &["commit", "-q", "--allow-empty", "-m", "a"], date1);
        git(d, &["checkout", "-q", "-b", "feature"], date1);
        git(d, &["commit", "-q", "--allow-empty", "-m", "b"], "2026-10-05T10:00:00Z");
        // a remote-tracking ref newer than anything local (work from another device, fetched)
        git(d, &["commit", "-q", "--allow-empty", "-m", "c"], "2026-10-09T10:00:00Z");
        git(d, &["update-ref", "refs/remotes/origin/main", "HEAD"], date1);
        git(d, &["reset", "-q", "--hard", "HEAD~1"], date1);
        git(d, &["remote", "add", "origin", "gh:Me/App.git"], date1);
        git(d, &["remote", "add", "mirror", "short:me/app"], date1);
        git(d, &["remote", "add", "other", "https://gitlab.com/me/app.git"], date1);
        git(d, &["config", "url.https://github.com/.insteadOf", "short:"], date1);

        let aliases = parse_ssh_config("Host gh\nHostName github.com\n");
        let info = read(d, &aliases).unwrap();
        assert_eq!(info.last_commit, Some("2026-10-05T10:00:00Z".parse().unwrap()));
        assert_eq!(info.last_remote_ref, Some("2026-10-09T10:00:00Z".parse().unwrap()));
        assert_eq!(info.github_slugs, vec!["me/app".to_string()], "origin and the insteadOf mirror are the same repo");

        let repo = |id, name: &str| Repo {
            id,
            name: name.into(),
            full_name: name.into(),
            owner: Owner { login: "me".into() },
            private: true,
            fork: false,
            archived: false,
            created_at: "2026-01-01T00:00:00Z".parse().unwrap(),
            pushed_at: None,
        };
        assert_eq!(match_repo(&info, &[repo(1, "me/other"), repo(2, "ME/app")]), Match::Unique(2));
        assert_eq!(match_repo(&info, &[repo(1, "me/other")]), Match::None);
        let two = LocalInfo { github_slugs: vec!["me/app".into(), "me/other".into()], ..Default::default() };
        assert_eq!(match_repo(&two, &[repo(1, "me/other"), repo(2, "me/app")]), Match::Ambiguous(vec![1, 2]));
    }

    #[test]
    fn the_fingerprint_changes_with_commits_fetches_and_remotes_only() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let date = "2026-10-01T10:00:00Z";
        git(d, &["init", "-q", "-b", "main"], date);
        git(d, &["commit", "-q", "--allow-empty", "-m", "a"], date);
        let f0 = fingerprint(d).unwrap();
        assert_eq!(fingerprint(d).unwrap(), f0, "nothing happened");
        std::fs::write(d.join("notes.txt"), "work in progress").unwrap();
        assert_eq!(fingerprint(d).unwrap(), f0, "editing files isn't activity");
        // a commit on a nested branch name only touches a sub-folder of refs/heads
        git(d, &["checkout", "-q", "-b", "feat/x"], date);
        let f1 = fingerprint(d).unwrap();
        git(d, &["commit", "-q", "--allow-empty", "-m", "b"], date);
        let f2 = fingerprint(d).unwrap();
        assert_ne!(f2, f1);
        git(d, &["update-ref", "refs/remotes/origin/main", "HEAD"], date);
        let f3 = fingerprint(d).unwrap();
        assert_ne!(f3, f2, "a fetched remote ref");
        git(d, &["remote", "add", "origin", "https://github.com/me/app.git"], date);
        assert_ne!(fingerprint(d).unwrap(), f3, "a new remote");
        assert_eq!(fingerprint(&d.join("missing")), None);
    }

    #[test]
    fn not_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path(), &HashMap::new()).is_err());
    }
}
