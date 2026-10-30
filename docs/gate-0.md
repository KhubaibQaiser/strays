# Gate 0 — interim results (2026-10-30)

Machine: MacBook (Apple Silicon, arm64), macOS 26.5, 3024×1964 display at 2×. Other apps were running (Cursor, Chrome, Claude), so the CPU figures carry a few percent of noise.

## Results

| Check | Result | Evidence |
|---|---|---|
| Rive CLI can author a rigged character as text | ✅ Pass | `art/cat/scene.rml`; `rive --verify` 0 errors |
| Bones + skinning (tail on 3-bone chain) | ✅ Pass | smooth bend, `art/review/cat-gate0-timeline.png` |
| Idle loops: breathe, blink, tail sway, ear flick | ✅ Pass | 12 timeline frames all distinct; blinks visible |
| Colour data binding (CLI renderer) | ✅ Pass | `art/review/cat-gate0-colours.png` (cream, black, ginger, blue) |
| Colour data binding (web runtime in Tauri webview) | ✅ Pass | log `LOADED pet=N colours=4/4`; screenshot shows 3 recoloured cats |
| Unsigned `.riv` (no scripts) plays without watermark | ✅ Pass | screenshot of the overlay |
| Rive WASM bundled locally, strict CSP | ✅ Pass | `dist/assets/rive-*.wasm`; CSP `default-src 'self'` |
| Transparent always-on-top overlay windows | ✅ Pass | screenshot |
| Click-through + ⌥-click (real OS events) | ⏸ Blocked | Accessibility not yet granted to the agent's host app (`AXIsProcessTrusted: false`); test ready in `tools/overlay-harness/click-test.sh` |
| Cursor/modifier polling with no permission prompt | ✅ Pass (partial) | ran without any TCC prompt; ⌥ detection itself is verified by the blocked test |
| **CPU budget < 1% (3 idle pets)** | ❌ **Fail** | see below |
| Memory < 200 MB | ✅ Layout B / ❌ Layout A | see below |
| macOS 12 compatibility | ⏳ Not run | needs a macOS 12 VM (manual approval) |

## Performance (release build, 3 cats, `tools/overlay-harness/perf.sh`, 30 s samples)

Total = app + its WebKit processes + the extra WindowServer CPU over its baseline.

| Layout | fps cap | Total CPU | Memory |
|---|---|---|---|
| A (window per pet) | 60 | 51.8% | 436 MB |
| A | 30 | 38.1% | 370 MB |
| A | 20 | 29.8% | 352 MB |
| A | 10 | 19.4% | 312 MB |
| B (one window per screen) | 60 | 34.9% | 247 MB |
| B | 30 | 19.9% | 195 MB |
| B | 20 | 20.9% | 187 MB |
| B | 10 | 11.3% | 193 MB |
| B | 1 | 6.0% | 188 MB |

Breakdown at B/20 fps: WebKit GPU process ~9%, WebContent ~8%, app (Rust poller + IPC) ~4%.

**Reading:** the frame cap works (Rive reports 60 → 20 fps), so each frame is expensive: `@rive-app/canvas` draws vector paths with Canvas2D. Layout B beats A on CPU and memory. Even the 1 fps baseline (~3–6%) is over budget, partly from the poller calling Tauri's monitor/cursor APIs 4×/s.

## Options to decide

1. **WebGL2 renderer** (`@rive-app/webgl2`, Rive's GPU renderer). Cheapest test: one package install. Probably lowers WebContent CPU a lot; unknown whether it gets near 1%.
2. **Native overlay on macOS**: Rive's Apple runtime (Metal) in native windows, with Tauri (or SwiftUI) for settings. Likely the most efficient. It changes the stack: the overlay becomes per-platform code.
3. **Tune the current path**: render only while something moves, idle at ≤10 fps, a cheaper poller. Unlikely to reach 1% alone, but it combines with 1 or 2.
4. **Relax the budget**, e.g. < 3% average. A product decision, because it affects battery.
