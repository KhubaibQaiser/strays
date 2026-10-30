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

## Native overlay (Rive Apple runtime 6.28.1, Metal) — `native-overlay/`

Measured by process CPU time over 15–32 s (`ps` cputime delta), release build. Windows are `NSPanel`s, floating level, all Spaces, `sharingType = .none`.

| Setup | CPU | Memory |
|---|---|---|
| Panels + ⌥ poller, 0 pets | **0.07%** | — |
| 1 pet paused (static frame) | 0.07% | — |
| 1 pet, 10 fps | 1.73% | — |
| 1 pet, 15 fps | 2.87% | — |
| 1 pet, 30 fps | 4.67% | — |
| 1 pet, 60 fps | 8.00% | — |
| 3 pets, 15 fps | 5.87% | ~45 MB total |
| 3 pets, 30 fps | 11.33% | ~45 MB total |
| 3 pets, 30 fps, 0.6 s burst every 8 s | 4.78% | |
| 3 pets, 30 fps, 1.2 s burst every 8 s | 7.69% | |

Findings:
- Memory is ~4× lower than the webview (45 MB vs ~190 MB), and the poller is essentially free (0.07% vs ~3%).
- Rendering costs about 0.15% CPU per pet per frame per second. **Continuous animation of 3 pets cannot meet < 1%** at any smooth frame rate.
- The frame cap only takes effect when set on the `RiveView` after it is added to a window, and it needs macOS 14 (on 13 the runtime draws at display rate).
- **Rive's Apple runtime requires macOS 13.1+**, which raises the spec's macOS 12 minimum.
- Bursts (pause between short plays) helped less than the duty cycle predicts. Pause/play in the high-level API seems to keep rendering for a while, or to cost something on resume. Needs the low-level API (manual `advance` on our own timer) to test properly.
- `sharingType = .none` keeps pets out of `screencapture` (verified: pets invisible in screenshots unless the test flag re-enables capture).
- Colour binding: `ColorProperty.set(red:green:blue:)` turned colours white; assigning `value = NSColor(...)` works.
