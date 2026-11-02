#!/usr/bin/env python3
"""Fake GitHub API for Strays tests and `--fake-github` dev runs (spec §14.2). Standard library only.

Usage: server.py [--port N]   prints "PORT <n>" once listening.
Pick a scenario with  POST /_scenario  {"name": "..."}  (default "ok"). Scenarios:
  ok            250 repos over 3 pages (sorted by pushed_at, newest first), ETag + 304, expiry header
  unauthorized  401 everywhere
  rate_limit    403 with x-ratelimit-remaining: 0 and a reset time
  secondary     403 secondary rate limit with Retry-After: 60
  too_many      429 with Retry-After: 30
  no_release    /releases/latest is 404 (nothing published yet)
Fixed repos for /repositories/{id}: 1 normal, 2 deleted (404), 3 archived, 4 renamed to me/renamed.
/repos/{owner}/{repo}/releases/latest answers v9.9.9 and must come without a token (update checks are anonymous).
"""
import json
import sys
import time
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

TOKEN = "github_pat_fake"
ETAG = '"fake-etag-1"'
NOW = datetime(2026, 10, 30, 12, 0, tzinfo=timezone.utc)
EXPIRY = "2026-11-29 13:28:15 UTC"
state = {"scenario": "ok", "requests": []}


def iso(t):
    return t.strftime("%Y-%m-%dT%H:%M:%SZ")


def repo(i, name=None, archived=False, pushed=None):
    return {
        "id": i,
        "name": name or f"repo-{i}",
        "full_name": f"me/{name or f'repo-{i}'}",
        "owner": {"login": "me"},
        "private": i % 2 == 0,
        "fork": i % 10 == 0,
        "archived": archived,
        "created_at": iso(NOW - timedelta(days=400 - i)),
        "pushed_at": iso(pushed or NOW - timedelta(hours=i * 6)),
    }


# repo i is pushed i*6 hours before NOW, so page 1 (1-100) is the newest
REPOS = [repo(i) for i in range(1, 251)]
FIXED = {1: repo(1), 3: repo(3, archived=True), 4: repo(4, name="renamed")}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, status, body=None, headers=()):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("github-authentication-token-expiration", EXPIRY)
        self.send_header("x-ratelimit-limit", "5000")
        self.send_header("x-ratelimit-remaining", "4990")
        self.send_header("x-ratelimit-reset", str(int(time.time()) + 3600))
        for k, v in headers:
            self.send_header(k, v)
        data = b"" if body is None else json.dumps(body).encode()
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        if self.path == "/_scenario":
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            state["scenario"] = body["name"]
            state["requests"] = []
            return self.send(200, {"ok": True})
        self.send(404, {"message": "Not Found"})

    def do_GET(self):
        url = urlparse(self.path)
        if url.path == "/_requests":
            return self.send(200, state["requests"])
        state["requests"].append(self.path)
        if url.path.endswith("/releases/latest"):
            if self.headers.get("Authorization"):
                return self.send(400, {"message": "update checks must not send the token"})
            if state["scenario"] == "no_release":
                return self.send(404, {"message": "Not Found"})
            return self.send(200, {"tag_name": "v9.9.9", "html_url": "https://github.com" + url.path.removeprefix("/repos").removesuffix("/latest") + "/tag/v9.9.9"})
        if self.headers.get("Authorization") != f"Bearer {TOKEN}":
            return self.send(401, {"message": "Bad credentials"})
        sc = state["scenario"]
        if sc == "unauthorized":
            return self.send(401, {"message": "Bad credentials"})
        if sc == "rate_limit":
            reset = str(int(NOW.timestamp()) + 600)
            self.send_response(403)
            self.send_header("x-ratelimit-remaining", "0")
            self.send_header("x-ratelimit-reset", reset)
            self.send_header("Content-Length", "2")
            self.end_headers()
            return self.wfile.write(b"{}")
        if sc == "secondary":
            return self.send(403, {"message": "You have exceeded a secondary rate limit"}, [("Retry-After", "60")])
        if sc == "too_many":
            return self.send(429, {"message": "Too many requests"}, [("Retry-After", "30")])

        if url.path == "/user":
            return self.send(200, {"login": "me"})
        if url.path == "/user/repos":
            q = parse_qs(url.query)
            per = int(q.get("per_page", ["30"])[0])
            page = int(q.get("page", ["1"])[0])
            if page == 1 and self.headers.get("If-None-Match") == ETAG:
                return self.send(304)
            items = REPOS[(page - 1) * per : page * per]
            last = (len(REPOS) + per - 1) // per
            base = f"http://{self.headers['Host']}/user/repos?sort=pushed&per_page={per}"
            links = []
            if page < last:
                links.append(f'<{base}&page={page + 1}>; rel="next"')
                links.append(f'<{base}&page={last}>; rel="last"')
            headers = [("ETag", ETAG)] if page == 1 else []
            if links:
                headers.append(("Link", ", ".join(links)))
            return self.send(200, items, headers)
        if url.path.startswith("/repositories/"):
            rid = int(url.path.rsplit("/", 1)[1])
            if rid in FIXED:
                return self.send(200, FIXED[rid])
            return self.send(404, {"message": "Not Found"})
        self.send(404, {"message": "Not Found"})


def main():
    port = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 0
    server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    print(f"PORT {server.server_address[1]}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
