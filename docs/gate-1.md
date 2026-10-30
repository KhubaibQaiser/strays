# Gate 1 — core logic (2026-10-31)

All the logic lives in `app/core` (`strays-core`), a library with no Tauri dependency. Run the tests with `cargo test` in `app/core`. The live check is `cargo test -- --ignored live`.

| Area | Module | Tests |
|---|---|---|
| Mood and idle days (§5.2, §5.3): thresholds, paces, frozen time, local weekends, a clock that goes backwards | `mood.rs` | Unit tests, plus 4 property tests. Idle days are never negative or longer than the span. A later evaluation time never lowers them, so a failed sync can't make a mood worse. More frozen time or skipped weekends never raises them, and neither does newer activity. |
| Pet record and state machine (§5.1, §5.5, §5.6, §11.1): adopt, snooze and wake, pause and resume, release → holiday (eligibility and cooldown), graduate, let go and undo, deadline ticks, slots | `pet.rs` | Unit tests |
| Settings, with validation, and defaults for missing fields | `settings.rs` | Unit tests |
| Atomic JSON store. Corrupt files, or files from a newer version, are skipped and kept. | `store.rs` | Unit tests |
| GitHub client and sync (§6): fine-grained tokens only, allowlisted host, no redirects, ETag/304, incremental and full listings, deletion detected by repo id, renames, archives, new-project detection, 401, rate limits and network errors, backoff | `github.rs` | Fake server (`tools/fake-github/server.py`) and live |
| Local git (§5.4): newest commit on local branches and on remote-tracking refs; matching remotes to GitHub (https, ssh, scp-style, ssh host aliases, `insteadOf`) | `localgit.rs` | Git fixture repos |

**Results:** all 36 tests pass, and clippy reports no warnings. The live test ran with the read-only token on 2026-10-31:
- the token was validated and its expiry read
- all 96 repos were listed in push order
- a repeat request got a 304
- a repo was looked up by id

**Decision:** repos owned by another account, where the user is only a collaborator, are not supported in v1. They can be tracked as local folders (spec §6.1).

**Moved to Gate 2**, because these are app-shell work rather than core logic:
- Keychain storage
- Sync and deadline scheduling
- Display rotation (§5.7)
- Watching folders for new repos
- Nudges
- Wiring the core into the Tauri app

The per-field `updatedAt` from §11.1 is deferred until cross-device sync is designed.
