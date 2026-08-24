# Mute-your-boss Technical Solution Document (TSD)

- Version: v0.2
- Status: Draft
- Last updated: 2026-08-21
- Related document: See `PRD.md` for product requirements.

> This document answers "how to do it": architecture, selection, specs, and trade-offs. Requirements themselves are governed by `PRD.md`.

---

## 1. Overall Architecture

```
┌─────────────────────────────┐
│       GUI / CLI / SDK       │
└────────────┬────────────────┘
             │ Unified API (gRPC/HTTP, localhost)
┌────────────▼────────────────┐
│  API Gateway (Go)           │
└────────────┬────────────────┘
             │
┌────────────▼────────────────┐
│      myb-server             │  ← gRPC server, assembles all crates
└────────────┬────────────────┘
             │
┌────────────▼────────────────┐
│      myb-core               │  ← FocusSession orchestration
│  ├─ Session Manager         │
│  ├─ Audio / Volume / KWS /  │
│  │  Policy / EventLog traits │
│  └─ Fail-safe / Watchdog    │
└─────────────────────────────┘
             │
    ┌────────┴────────┬────────────────┬────────────────┐
    ▼                 ▼                ▼                ▼
┌──────────┐  ┌──────────────┐  ┌──────────┐  ┌─────────────┐
│myb-audio-│  │myb-volume-   │  │myb-kws   │  │myb-policy   │
│capture   │  │control       │  │          │  │             │
│Win:WASAPI│  │Win:IAudio    │  │sherpa-   │  │YAML / match │
│mac:SCK/  │  │mac:HAL/Tap   │  │onnx      │  │/ action     │
│Lin:PW    │  │Lin:PW/PA     │  │          │  │             │
└──────────┘  └──────────────┘  └──────────┘  └─────────────┘
                               ┌─────────────┐
                               │myb-event-log│
                               │JSONL/SQLite │
                               └─────────────┘
```

Layering principle:

- **myb-server** is the only gRPC server binary. It assembles `myb-core`, `myb-audio-capture`, `myb-volume-control`, `myb-kws`, `myb-policy`, and `myb-event-log` and exposes the unified API.
- **myb-core is platform-agnostic**: it defines all traits (`AudioCapture`, `VolumeController`, `KwsEngine`, `PolicyEngine`, `EventLog`) and orchestrates a `FocusSession` through dependency injection. It holds session state, fail-safe logic, and the watchdog.
- **Specialized crates implement Core traits**: `myb-kws` handles sherpa-onnx keyword spotting; `myb-policy` handles YAML policy matching; `myb-event-log` handles persistence; `myb-audio-capture` / `myb-volume-control` handle platform-specific audio capture and volume control.
- **API layer separation**: the upper-layer interface is consistent across platforms. The GUI only calls the API and never touches platform logic directly.
- **Testability**: `myb-core` can be unit-tested with mock implementations of every trait, without starting a real meeting app.

## 2. Platform Adaptation Plan

### 2.1 Audio Capture (per-process loopback)

| Platform | Solution | Constraints |
|----------|----------|-------------|
| Windows | WASAPI process loopback capture (`AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS`; native per-process on Win11 22H2+; lower versions fall back to session enumeration + process-specific audio session filtering) | Per-session volume enumeration from Win10 1809; per-process capture compatibility matrix needs to be verified case by case |
| macOS | Audio Process Tap (CoreAudio, macOS 14.2+) or ScreenCaptureKit audio stream | Requires user authorization (Screen & System Audio Recording permission); not supported below 14.2, prompt to upgrade |
| Linux | PipeWire (preferred) capture by node; fallback PulseAudio capture by sink-input | Pure ALSA environments not supported, prompt accordingly |

Capture output is uniformly resampled to **16 kHz / mono / float32 PCM** and fed into the KWS pipeline.

### 2.2 Volume Control (per-process)

| Platform | Solution |
|----------|----------|
| Windows | WASAPI `ISimpleAudioVolume` (per-session) |
| macOS | Audio Process Tap volume attributes / CoreAudio HAL |
| Linux | PipeWire `wpctl` / PulseAudio sink-input volume |

Volume switching uses ~200ms fade in / fade out to avoid popping.

### 2.3 Trade-off: Capture-then-Control vs Virtual Audio Device

- **Selected solution**: Direct capture + direct control of the target process volume. Pros: zero install intrusion, no driver, no need to restart the meeting app. Cons: strongly depends on relatively new per-process APIs on each platform.
- **Alternative**: Virtual audio card routing (meeting app output → virtual device → app forwards/drops → real speaker). Pros: fully self-determined capture and control, small platform differences. Cons: requires installing a kernel driver / virtual device (macOS needs kext or third-party driver; Windows needs signed driver), high installation and trust cost, and requires the user to change the meeting app's audio output device. **Not adopted for MVP**; fall back only if platform API compatibility issues cannot be resolved.

## 3. Keyword Spotting Pipeline (KWS)

### 3.1 Selection: sherpa-onnx KWS

Use **sherpa-onnx Keyword Spotting** (zipformer / transducer family streaming models):

- **Keywords are text-configured**: keywords are written in a config file (Chinese written as pinyin with tones, English written as phonemes / letter sequences). The engine performs acoustic matching against the vocabulary **without training a model for each keyword and without networking** — this is its decisive advantage over openWakeWord (each word requires offline training) and Porcupine (custom words require cloud training).
- **Native streaming**: frame-by-frame detection scores with sub-second latency.
- **Pure CPU real-time operation**: model size is tens of MB, low single-core usage, no GPU dependency.
- Existing Chinese-optimized models (wenetspeech, etc.) support Chinese-English mixed configuration.
- Official SDKs are available in Rust / Go / C / C++ / Python; `myb-kws` uses the official Rust API, no need for a custom FFI.

### 3.2 Pipeline Spec

```
Audio Capture → Ring Buffer (16kHz/mono/f32) → KWS (sherpa-onnx, streaming)
                                                      │ score ≥ threshold
                                           Policy Engine (policy matching)
```

- Each session builds a keyword vocabulary from its policy's keyword set; the vocabulary is hot-updated when policies change, without restarting capture.
- Each keyword can be configured with a detection threshold (default to model-recommended value) to balance false positives / misses.
- Event output: `{keyword, confidence, timestamp}`, no transcription text produced.
- Latency budget breakdown (end-to-end ≤ 1s): capture buffering ≤ 200ms + KWS detection ≤ 300ms + policy matching and volume execution ≤ 100ms, with margin.

### 3.3 Environment Requirements and Exception Handling

- No GPU required; any modern x86_64 / arm64 CPU is sufficient.
- Load model and self-check on startup; if model is missing / corrupted, refuse to start the session and provide a readable error.
- If high system load causes the detection thread to fall behind: drop stale buffers and notify of increased latency; on sustained anomaly enter fail-safe (keep normal volume and notify).

## 4. Policy Engine Spec

### 4.1 Configuration Format (YAML, persisted to user config directory)

```yaml
policies:
  - name: "roll-call"
    keywords: ["Zhang San", "Xiao Zhang"]  # required; Chinese auto-converted to pinyin, or write pinyin/phonemes manually for fine-tuning
    match:
      threshold: 0.6                     # detection threshold (0-1), can be overridden per keyword
      languages: ["zh", "en"]
    action:
      volume: 100                        # restore normal volume (0-100)
      duration_seconds: 30               # keep normal volume duration; renewed while speech continues
      then: "auto"                       # after timeout: auto=return to policy evaluation / mute=mute directly
  - name: "default"
    keywords: []
    action:
      volume: 0                          # when no match: mute
```

### 4.2 Matching and Execution Semantics

- Matching input is the KWS score stream; a score exceeding the threshold triggers an action with sub-second latency.
- Acoustic matching itself tolerates homophones / near-sounds to some extent; false positives / misses are handled by threshold adjustment, not by a second text-level matching pass.
- Multiple policies are matched in list order; the first hit takes effect; when there is no hit the default action is executed.
- Debounce: repeated hits of the same policy within `debounce_seconds` (default 5s) only refresh the duration.
- Event log: local SQLite or JSONL storage (timestamp, policy name, matched keyword, confidence), local only, one-click clear.

## 5. Unified API Design

### 5.1 Form

- Local gRPC (HTTP/2, good stream support) as primary, with HTTP/JSON gateway exposed for debugging; only listens on `127.0.0.1`, generates a local token for authentication on startup.
- API Gateway is implemented in Go, responsible for authentication, routing, protocol conversion, and rate limiting; `myb-server` is implemented in Rust and exposes the unified gRPC API, delegating all business logic to `myb-core`.
- SDK: first version prioritizes Rust SDK (used by CLI and Tauri GUI), interfaces defined by proto; other language SDKs generated as needed.

### 5.2 Interface Definition (Illustrative)

```proto
service MuteYourBoss {
  rpc ListAudioProcesses(Empty) returns (ProcessList);
  rpc StartSession(StartSessionReq) returns (Session);   // pid + policy
  rpc StopSession(SessionRef) returns (Empty);
  rpc SetVolume(SetVolumeReq) returns (Empty);            // pid + 0~100
  rpc GetEventStream(SessionRef) returns (stream Event);  // {policy, keyword, confidence, ts}
  rpc GetStatus(SessionRef) returns (Status);             // {state, volume, device, latency_ms}
  rpc ValidatePolicy(PolicyYaml) returns (ValidationResult);
}
```

The same proto definition applies to all three platforms; behavioral differences are considered bugs.

## 6. Language and Framework Selection

| Component | Selection | Rationale |
|-----------|-----------|-----------|
| API Gateway | Go | Mature gRPC/HTTP service, strong concurrency, simple deployment; only authentication and protocol conversion |
| myb-server | Rust | gRPC server binary; assembles all Rust crates and exposes the unified API |
| myb-core | Rust | Platform-agnostic session orchestration; defines all traits consumed via dependency injection |
| myb-kws | Rust | sherpa-onnx keyword spotting; implements the `KwsEngine` trait |
| myb-policy | Rust | YAML policy parsing and matching; implements the `PolicyEngine` trait |
| myb-event-log | Rust | Local event persistence; implements the `EventLog` trait |
| myb-audio-capture | Rust | Platform-specific audio capture; implements the `AudioCapture` trait |
| myb-volume-control | Rust | Platform-specific volume control; implements the `VolumeController` trait |
| GUI | Tauri | Small package size, low memory usage; frontend uses Web tech stack, native capabilities provided by Rust backend |
| KWS model | sherpa-onnx | Official Rust SDK, no custom FFI; streaming, pure CPU |

**Layering principles**:

- GUI / CLI / SDK are all consumers of the unified API and do not directly touch platform audio logic.
- API Gateway does not hold Session, KWS, audio capture, or other business state; it only forwards external requests to `myb-server`.
- `myb-server` is the only gRPC server binary. It wires together `myb-core` and all trait-implementing crates.
- `myb-core` is platform-agnostic. It defines `AudioCapture`, `VolumeController`, `KwsEngine`, `PolicyEngine`, and `EventLog` traits and orchestrates a `FocusSession`. It does not depend on any platform-specific crate.
- Each specialized crate (`myb-kws`, `myb-policy`, `myb-event-log`, `myb-audio-capture`, `myb-volume-control`) depends on `myb-core` and implements one or more traits.
- This structure makes every component independently testable with mock trait implementations.

**Notes**:

- Python no longer enters the main runtime path, and is retained only for auxiliary purposes such as model conversion and test scripts.
- From M1 onward implementation follows this layering; no "Python full-stack validation" path.

## 7. Stability and Security Mechanisms

- **Volume restore guardian**: an independent watchdog thread / process holds a snapshot of the target process's original volume; automatically restores when the main process crashes (heartbeat lost).
- **Fail-safe principle**: any uncertain state (detection engine exception, policy parsing failure, permission loss) returns to "normal volume" — better audible than missed.
- **Panic shortcut**: global hotkey (default `Ctrl+Alt+M`, configurable), immediately restores normal volume and pauses policies for 60s.
- **API security**: localhost + local token only; no remote access (remote control is an open question, see PRD §10).

## 8. Compatibility Matrix

| Platform | Minimum Version | Notes |
|----------|-----------------|-------|
| Windows | 10 1809 (volume control) / 11 22H2 (best per-process capture experience) | Per-process capture on lower versions needs verification, otherwise prompt to upgrade |
| macOS | 14.2 | Audio Process Tap hard requirement |
| Linux | PipeWire ≥ 0.3 (Ubuntu 22.04+, etc.) | PulseAudio fallback; pure ALSA not supported |

## 9. Model and Dependency Distribution

- KWS model size is only tens of MB, **bundled directly in the installer**, ready to use out of the box.
- Supports model file verification and later replacement with version updates.
- sherpa-onnx prebuilt libraries for all three platforms are distributed with the package; self-check on startup and provide readable errors.

## 10. Technical Risks

1. **Platform API compatibility**: Windows per-process loopback needs case-by-case verification under different meeting apps (Tencent Meeting / Feishu); some meeting apps may use a custom audio stack that bypasses the default session → top verification item for M1.
2. **macOS version threshold**: below 14.2 per-process capture is impossible; can only prompt to upgrade or fall back to virtual audio device solution.
3. **KWS detection rate and false triggers**: dialects, accents, and far-field pickup cause misses; overly short or phonetically common keywords cause false positives → mitigated by configurable thresholds, recommended keyword length, and fail-safe; before release, build a keyword detection test set (quiet / noisy × Mandarin / accent).

## 11. Milestones and Technical Verification Items

| Phase | Technical Verification Items (DoD) |
|-------|------------------------------------|
| M1 | Windows per-process capture verified on Tencent Meeting / Feishu; sherpa-onnx KWS pipeline end-to-end trigger ≤ 1s; automatic volume restore after crash |
| M2 | Cross-platform API consistency tests pass for the same proto; macOS / Linux capture and volume adapters complete; policy engine unit tests cover matching / debounce / priority |
| M3 | GUI completes all P0/P1 functions through unified API; installers produced for all three platforms; 8-hour stability test passes |

## 12. Technical Open Questions

1. Default detection thresholds for each keyword and coverage of Chinese automatic pinyin conversion need to be tuned and verified.
2. Multi-session architecture reservation (managing multiple meetings simultaneously): Session Manager is implemented as single-session, but interfaces are designed for multi-session.
