# Mute-your-boss Development Plan

- Version: v0.1
- Status: Awaiting Review
- Related documents:
  - `PRD.md`: Product requirements
  - `TSD.md`: Technical solution
  - `docs/dev-plan.md`: This document

> This document answers "what to do and when": detailed tasks, dependencies, acceptance criteria, and risks broken down by milestone.

---

## 1. Overview

### 1.1 Goals

Deliver Mute-your-boss in three phases using the technology stack defined in `TSD.md`:

- **M1**: Validate the core pipeline on Windows: select process → audio capture → KWS detection → trigger volume → crash recovery.
- **M2**: Unified API available on all three platforms (Windows / macOS / Linux), policy engine refined.
- **M3**: Tauri GUI productization, installers produced for all three platforms, stability tests passed.

### 1.2 Finalized Technology Stack

| Component | Technology |
|-----------|------------|
| GUI | Tauri (Web frontend + Rust backend) |
| API Gateway | Go (gRPC + HTTP/JSON) |
| gRPC Server | `myb-server` (Rust) |
| Session Orchestration | `myb-core` (Rust) |
| Keyword Spotting | `myb-kws` + sherpa-onnx official Rust API |
| Policy Engine | `myb-policy` (Rust) |
| Event Log | `myb-event-log` (Rust) |
| Audio Capture | `myb-audio-capture` + platform native APIs |
| Volume Control | `myb-volume-control` + platform native APIs |
| Configuration format | YAML |
| Inter-process communication | gRPC over localhost / Unix Domain Socket |

---

## 2. Milestone Overview

| Phase | Core Deliverable | Acceptance Criteria |
|-------|------------------|---------------------|
| M1 | Windows core runnable prototype | Tencent Meeting / Feishu end-to-end trigger ≤ 1s; volume restored after crash |
| M2 | Cross-platform platform crates + myb-core + Go Gateway | Consistent API behavior across platforms; policy engine unit tests; macOS / Linux adapters complete |
| M3 | Tauri GUI + installers for all three platforms | US-1 ~ US-7 all accepted; 8-hour stability test passed |

---

## 3. M1: Core Pipeline Validation (Windows First)

### 3.1 Goal

Implement a minimum usable prototype on Windows: select a Tencent Meeting / Feishu process, perform keyword detection on its audio, restore volume on hit, and restore mute or original volume after timeout / crash.

### 3.2 Task List

#### M1.1 Engineering Skeleton

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M1.1.1 | Initialize monorepo: create `/gateway` (Go), `/crates/myb-core`, `/crates/myb-audio-capture`, `/crates/myb-volume-control`, `/crates/myb-kws`, `/crates/myb-policy`, `/crates/myb-event-log`, `/crates/myb-server`, `/proto` | None | All crates and gateway can be compiled independently; root `Makefile` provides `build`, `test`, `fmt` | [x] |
| M1.1.2 | Define proto file: `MuteYourBoss` service, Process / Session / Event messages | None | `buf generate` can produce Go and Rust code; CI checks proto changes | [x] |
| M1.1.3 | Set up `myb-server`: `tokio` + `tonic` gRPC server, logging and config loading | M1.1.2 | `cargo run -p myb-server` starts and listens on localhost gRPC | [x] |
| M1.1.4 | Define all traits in `myb-core`: `AudioCapture`, `VolumeController`, `KwsEngine`, `PolicyEngine`, `EventLog` | M1.1.1 | Traits are platform-agnostic; include mock-friendly abstractions | [x] |
| M1.1.5 | Initialize stub crates `myb-audio-capture`, `myb-volume-control`, `myb-kws`, `myb-policy`, `myb-event-log` | M1.1.1, M1.1.4 | All compile and depend on `myb-core` traits; provide mock implementations for unit tests | [x] |

#### M1.2 Audio Capture (Windows)

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M1.2.1 | Research WASAPI process loopback capture API in `windows-rs` | None | Produce a minimal compilable example; confirm Win10 / Win11 differences | [ ] |
| M1.2.2 | Implement `AudioCapture` trait for Windows in `crates/myb-audio-capture` | M1.1.4, M1.2.1 | Can capture process audio by PID and output 16kHz / mono / f32 PCM | [ ] |
| M1.2.3 | Implement process enumeration in `crates/myb-audio-capture` | M1.2.2 | Unit tests cover common process filtering logic | [ ] |
| M1.2.4 | Field verification with Tencent Meeting and Feishu: confirm per-process capture feasibility | M1.2.2 | Produce compatibility report; if it fails, record fallback plan | [ ] |

#### M1.3 Volume Control (Windows)

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M1.3.1 | Implement `VolumeController` trait for Windows in `crates/myb-volume-control` | M1.1.4 | Can set volume 0–100 by PID; supports ~200ms fade in / fade out | [ ] |

#### M1.4 KWS Integration (myb-kws)

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M1.4.1 | Integrate sherpa-onnx Rust crate in `myb-kws`; load KWS model and self-check | None | `cargo build` passes; readable error if model is missing on startup | [ ] |
| M1.4.2 | Implement keyword vocabulary construction in `myb-kws`: extract keywords from YAML policies and generate sherpa-onnx vocabulary | M1.4.1 | Supports Chinese (pinyin with tones) + English mixed configuration | [ ] |
| M1.4.3 | Implement `KwsEngine` trait in `myb-kws`: consume any `AudioCapture` stream, output `{keyword, confidence, timestamp}` | M1.4.2, M1.1.4 | Can stably detect keywords from a mock audio stream in unit tests | [ ] |
| M1.4.4 | Keyword latency and detection rate test: quiet environment Mandarin, ≥ 95% detection, ≤ 1s latency | M1.4.3, M1.2.2 | Produce test report; record threshold tuning recommendations | [ ] |

#### M1.5 Policy Engine (myb-policy)

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M1.5.1 | Implement YAML policy parsing and validation in `myb-policy` | M1.1.4 | Invalid policies return clear errors; covered by unit tests | [ ] |
| M1.5.2 | Implement `PolicyEngine` trait in `myb-policy`: list-order matching, threshold judgment, action execution | M1.5.1 | After keyword hit, return correct `VolumeDecision` | [ ] |
| M1.5.3 | Implement debounce and renewal in `myb-policy`: repeated hits within the same policy debounce window refresh duration | M1.5.2 | Unit tests cover multiple hits within a 5s window | [ ] |

#### M1.6 Event Log (myb-event-log)

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M1.6.1 | Implement `EventLog` trait in `myb-event-log` | M1.1.4 | Append / recent / clear operations work; persists to JSONL or SQLite | [ ] |
| M1.6.2 | Add query API and one-click clear endpoint | M1.6.1 | gRPC `GetEventStream` / status can read from event log | [ ] |

#### M1.7 Core Orchestration (myb-core)

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M1.7.1 | Implement `FocusSession`: wire `AudioCapture` → `KwsEngine` → `PolicyEngine` → `VolumeController` + `EventLog` | M1.1.4, M1.4.3, M1.5.2, M1.6.1 | A keyword hit from a mock stream changes the mock volume | [ ] |
| M1.7.2 | Implement volume restore guardian in `myb-core` | M1.3.1, M1.7.1 | Target process volume is restored after manually killing the server | [ ] |
| M1.7.3 | Implement fail-safe: restore volume on KWS / policy / audio capture anomaly | M1.7.2 | Simulate any subsystem crash, volume returns to safe level | [ ] |
| M1.7.4 | Handle user manually changing volume: detect external changes and pause automatic control | M1.3.1 | State becomes paused after manual change; user decides whether to resume control | [ ] |

#### M1.8 API Gateway (Go)

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M1.8.1 | Initialize Go gateway: `gin/echo` HTTP + `grpc-go` client, listen only on 127.0.0.1 | M1.1.2 | `go run` starts; generates random token for authentication | [ ] |
| M1.8.2 | Implement forwarding layer: HTTP/JSON ↔ gRPC ↔ myb-server | M1.8.1, M1.1.3 | All interfaces reachable via Postman / curl | [ ] |
| M1.8.3 | Implement SSE / Websocket bridge for `GetEventStream` (easy frontend debugging) | M1.8.2 | Frontend can receive event stream | [ ] |

#### M1.9 CLI (Rust or Go)

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M1.9.1 | Implement minimal CLI: list / start / stop / status | M1.8.2 | Command line can fully operate one focus session | [ ] |

### 3.3 M1 Acceptance Criteria

- [ ] Per-process audio capture succeeds on Tencent Meeting or Feishu on Windows.
- [ ] After speaking the configured keyword, the target process volume is restored within ≤ 1s.
- [ ] If no further hit occurs within the duration, volume automatically returns to 0 or original volume (per policy).
- [ ] Manually killing myb-server automatically restores the target process volume.
- [ ] CLI can complete one full session operation.

---

## 4. M2: Cross-Platform and API Consistency

### 4.1 Goal

Complete macOS / Linux adapters in `crates/myb-audio-capture` and `crates/myb-volume-control`; ensure consistent unified API behavior across all three platforms; policy engine covered by unit tests; provide installation / startup scripts.

### 4.2 Task List

#### M2.1 macOS Platform Adaptation

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M2.1.1 | Implement macOS Audio Process Tap audio capture in `crates/myb-audio-capture` | M1.2.2 | Can capture target process audio on macOS 14.2+ | [ ] |
| M2.1.2 | Implement macOS CoreAudio HAL volume control in `crates/myb-volume-control` | M1.3.1 | Can adjust process volume with fade in / fade out | [ ] |
| M2.1.3 | Permission guidance: detect Screen & System Audio Recording authorization on first launch | M2.1.1 | Provide clear guidance when unauthorized, automatically continue after authorization | [ ] |
| M2.1.4 | Field verification with Tencent Meeting / Feishu / Zoom / Teams macOS versions | M2.1.1, M2.1.2 | Produce compatibility report | [ ] |

#### M2.2 Linux Platform Adaptation

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M2.2.1 | Implement PipeWire capture by node in `crates/myb-audio-capture` | M1.2.2 | Verified on Ubuntu 22.04+ / Fedora | [ ] |
| M2.2.2 | Implement PulseAudio sink-input fallback in `crates/myb-audio-capture` | M2.2.1 | Automatically degrades when PipeWire is unavailable | [ ] |
| M2.2.3 | Implement PipeWire / PulseAudio volume control in `crates/myb-volume-control` | M1.3.1 | Volume adjustment and fade in / fade out work normally | [ ] |
| M2.2.4 | Pure ALSA environment detection and friendly prompt | M2.2.2 | Detect on startup and prompt that it is unsupported | [ ] |

#### M2.3 API Consistency

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M2.3.1 | Write cross-platform API consistency test suite | M1.8.2 | Covers list / start / stop / set_volume / event_stream / status | [ ] |
| M2.3.2 | Run Windows / macOS / Linux consistency tests in CI | M2.3.1, M2.1.4, M2.2.4 | Tests pass on all three platforms | [ ] |
| M2.3.3 | Improve error codes and messages: missing permissions, process exit, missing model, etc. | M1.8.2 | Each error scenario returns a clear, readable error | [ ] |

#### M2.4 Policy Engine Refinement

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M2.4.1 | Unit tests cover policy matching, priority, debounce, renewal, timeout | M1.5.3 | Coverage ≥ 80% | [ ] |
| M2.4.2 | Implement Chinese automatic pinyin conversion, support manual pinyin / phoneme fine-tuning | M1.5.1 | Unit tests cover common names and module names | [ ] |
| M2.4.3 | Event log persistence tuning: JSONL vs SQLite, rotation, retention | M1.6.1 | Query / clear / retention policies work under load | [ ] |
| M2.4.4 | Hot policy update: modify policy at runtime without restarting session | M1.5.2 | Changes to YAML take effect automatically or clearly prompt a restart | [ ] |

#### M2.5 SDK and CLI

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M2.5.1 | Publish Rust SDK crate (local path dependency) | M2.3.2 | SDK example runs | [ ] |
| M2.5.2 | CLI supports policy editing and import / export | M2.4.1 | `myb policy add/edit/export/import` available | [ ] |
| M2.5.3 | Provide startup script: one-click start gateway + myb-server | M1.8.2 | Windows `.bat` / macOS Linux `.sh` | [ ] |

### 4.3 M2 Acceptance Criteria

- [ ] A complete focus session can be completed on Windows / macOS / Linux.
- [ ] The same proto interfaces behave consistently across all three platforms; consistency tests pass.
- [ ] Policy engine unit tests cover matching, debounce, and priority.
- [ ] Event log can be queried and cleared.
- [ ] Installation / startup script can start gateway + core with one click.

---

## 5. M3: Tauri GUI Productization

### 5.1 Goal

Implement the Tauri GUI based on the unified API, complete US-1 ~ US-7 from the PRD, and produce installers for all three platforms.

### 5.2 Task List

#### M3.1 GUI Foundation

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M3.1.1 | Initialize Tauri project: frontend framework selection (recommended React + TypeScript) | M2.3.2 | `pnpm tauri dev` runs | [ ] |
| M3.1.2 | Integrate Rust SDK / directly call Go gateway HTTP API | M2.5.1, M2.3.2 | Frontend can get process list and status | [ ] |
| M3.1.3 | Implement global Panic shortcut: `Ctrl+Alt+M` restores volume and pauses policies | M2.3.2 | Shortcut available on all three platforms | [ ] |

#### M3.2 GUI Feature Pages

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M3.2.1 | Process selection page: list view + highlight suspected meeting processes | M3.1.2 | Satisfies US-1 | [ ] |
| M3.2.2 | Main switch page: start / stop focus mode + status indicator | M3.2.1 | Satisfies US-2 | [ ] |
| M3.2.3 | Policy editor: add / delete / modify policies, keywords, thresholds, actions | M2.4.1 | Satisfies US-3 | [ ] |
| M3.2.4 | Event and status panel: real-time trigger notifications, current volume, latency | M2.4.3 | Satisfies US-4 | [ ] |
| M3.2.5 | Panic button and permission guidance dialog | M3.1.3 | Satisfies US-5, US-6 | [ ] |
| M3.2.6 | Advanced mode exposes unified API entry (for SDK / CLI use) | M2.5.1 | Satisfies US-7 | [ ] |

#### M3.3 Packaging and Stability

| ID | Task | Dependencies | Acceptance Criteria | Status |
|----|------|--------------|---------------------|--------|
| M3.3.1 | Configure Tauri packaging: Windows `.msi` / macOS `.dmg` / Linux `.AppImage` | M3.2.6 | CI produces installers for all three platforms | [ ] |
| M3.3.2 | 8-hour continuous operation stability test: no memory leaks, volume restore success rate ≥ 99% | M3.2.2 | Produce test report | [ ] |
| M3.3.3 | First-launch compliance statement dialog | PRD §11 | User must check the box before use | [ ] |
| M3.3.4 | Auto-update mechanism research and implementation (optional) | M3.3.1 | Can check for new version and prompt | [ ] |

### 5.3 M3 Acceptance Criteria

- [ ] US-1 ~ US-7 all accepted.
- [ ] Installers for all three platforms are automatically produced in CI.
- [ ] 8-hour stability test passed.
- [ ] Compliance statement shown on first launch.

---

## 6. Engineering Conventions and Collaboration

### 6.1 Branch Strategy

- `main`: always compiles, protected branch.
- `feature/M1-*`: M1 feature branches.
- `feature/M2-*`, `feature/M3-*`: subsequent milestone branches.
- Each task is merged into `main` via PR after completion; merge only after CI passes.

### 6.2 Code Conventions

| Language | Tools |
|----------|-------|
| Rust | `rustfmt` + `clippy` + `cargo-deny` |
| Go | `gofmt` + `golangci-lint` |
| TypeScript | `eslint` + `prettier` |
| Proto | `buf lint` |

### 6.3 CI Pipeline

- Every PR: format / lint / unit test
- Nightly: cross-platform consistency test
- M3 phase: automatic installer packaging

---

## 7. Testing Strategy

| Level | Tools | Coverage |
|-------|-------|----------|
| Unit tests | Rust `cargo test` / Go `go test` | Policy engine, config parsing, utility functions |
| Integration tests | Rust / Go integration tests | myb-server and Gateway end-to-end |
| Platform adapter tests | Manual + automated scripts | Audio capture, volume control, run on real machines / CI for all three platforms |
| KWS test set | Pre-recorded audio + live testing | Quiet / noisy × Mandarin / accent, record detection rate and false trigger rate |
| Stability tests | Long-running scripts | 8 hours continuous operation, monitor memory and volume restore |

---

## 8. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Windows per-process capture is ineffective for some meeting apps | High | M1.2.4 field verification first; if it fails, fallback to virtual audio device solution |
| macOS users below 14.2 cannot use the product | Medium | State compatibility threshold clearly; consider virtual audio device solution as v2 |
| sherpa-onnx Rust API maturity is insufficient | Medium | M1.4.1 integration POC first; if necessary switch to C API wrapped in Rust |
| Real-time audio latency exceeds target | Medium | M1.4.4 establish latency test set; ring buffer size tunable |
| Go ↔ Rust inter-process communication introduces complexity | Low | Use localhost gRPC initially; switch to Unix Domain Socket if necessary |
| Tauri global shortcuts are limited on some desktop environments | Low | Provide GUI button at the same time; graceful degradation for shortcuts |

---

## 9. Next Steps (This Week)

1. Review and confirm this Dev Plan.
2. Create GitHub / GitLab repository and basic CI configuration.
3. Initialize monorepo: (`/gateway`, `/crates/*`, `/proto`).
4. Start M1.1.1 ~ M1.1.5 engineering skeleton.

---

## 10. Appendix: Task ID Numbering Scheme

Task IDs use a hierarchical numbering scheme that mirrors the document structure:

- The first number is the milestone (`M1`, `M2`, `M3`).
- The second number is the section within that milestone (`M1.1`, `M1.2`, …).
- The third number is the individual task within that section (`M1.1.1`, `M1.1.2`, …).

For example, `M1.2.3` is the third task in the second section of Milestone 1. Dependency references use the same hierarchical IDs.
