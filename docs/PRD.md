# Mute-your-boss Product Requirements Document (PRD)

- Version: v0.3
- Status: Draft
- Last updated: 2026-08-21
- Related document: See `TSD.md` for the technical solution.

> This document answers only three questions: **what it is, why it matters, and what is needed**. It does not cover technical design, implementation specs, or trade-off decisions.

---

## 1. Product Overview (What it is)

**Mute-your-boss** is a desktop "online meeting focus assistant" for macOS / Linux / Windows. The user manually selects a running meeting process (e.g., Tencent Meeting, Feishu). The application performs local real-time keyword detection on that process's audio; only when a user-configured keyword is detected (e.g., their own name, being called on, being asked a question) does it restore the process volume to normal. At all other times the volume is automatically set to 0 (muted). Trigger strategies are freely configurable.

Core capability in one sentence: **Not about me → muted; about me → unmuted.**

> Compliance note: This product only detects audio from **meetings that the local user is participating in**, as a personal listening aid. By default it does not record, store, perform full transcription, or upload to the cloud. Users must comply with their company and meeting platform policies regarding recording and transcription.

## 2. Background and Value (Why it matters)

Knowledge workers (R&D, product managers, project managers, etc.) who spend long hours in online meetings but only need to follow a small amount of content relevant to them commonly face the dilemma of "afraid to mute, but no need to listen the whole time":

- Listening to the whole meeting: attention is consumed by large amounts of irrelevant content;
- Muting or leaving the meeting entirely: being called on or discussed without noticing is high risk.

Mute-your-boss solves this with "keyword-triggered volume": freeing attention when the user does not need to follow the whole meeting, while avoiding missed key information related to them.

### Typical Scenarios

1. **Meeting bystander**: Most of the daily stand-up or weekly sync is irrelevant. Enable mute mode, and volume is restored immediately when called on.
2. **Multitasking**: Write code while "attending" a meeting; switch back when keywords (your name, module you own) are detected.
3. **Custom attention**: Temporarily follow a topic (e.g., "release", "rollback") by adding its keywords to a policy.

## 3. Goals and Non-Goals

### 3.1 Goals

- The user can select a target meeting process from the UI and toggle "focus mode" on/off with one click.
- Local real-time keyword detection runs on CPU only, primarily for Mandarin and English, with low latency from keyword spoken to volume restored (target ≤ 1 second).
- Dynamically control the target process's playback volume based on configurable policies (normal / 0).
- Provide a unified cross-platform API with consistent behavior on macOS / Linux / Windows.
- Runs entirely locally; audio never leaves the machine.

### 3.2 Non-Goals

- No full meeting transcription, and no transcription text is generated (only whether a keyword is detected).
- No meeting summary or minutes generation (may be a future iteration).
- No speaker identification (it does not distinguish who is speaking; only keyword detection is performed).
- No dependency on any cloud API.
- No recording or saving of meeting audio files (only transient stream processing).

## 4. User Stories

| ID | User Story | Priority |
|----|------------|----------|
| US-1 | As a user, I want to select a currently running meeting process from a list (identified by process name / window title) so I can specify the focus target. | P0 |
| US-2 | As a user, I want the app to automatically mute the process after starting, and restore volume when my configured keywords are detected. | P0 |
| US-3 | As a user, I want to configure multiple policies (keyword list, restored volume, duration) | P0 |
| US-4 | As a user, I want to see "triggered" event notifications and current status so I know the app is working. | P1 |
| US-5 | As a user, I want a one-click "emergency unmute/mute" (panic button / shortcut) so I can manually take over at any time. | P0 |
| US-6 | As a user, I want clear prompts and guidance when system permissions are missing or the environment is unsupported. | P1 |
| US-7 | As a developer / advanced user, I want to perform all core operations through a unified API rather than the GUI. | P0 |

## 5. Functional Requirements (What is needed)

### 5.1 Process Discovery and Selection (P0)

- Enumerate currently audio-outputting processes, showing: process name, PID, window title (if available), current volume.
- Built-in matching presets for common meeting apps (Tencent Meeting / WeMeet, Feishu / Lark, Zoom, Teams, DingTalk, etc.), highlighting "suspected meeting processes".
- Users can manually select any process, not limited to the preset list.

### 5.2 Audio Capture (P0)

- Capture the audio output stream of the selected process without affecting other application audio.
- Supported on macOS / Linux / Windows with transparent, consistent capability for users.
- Provide clear guidance when system authorization is required (e.g., macOS audio capture permission).

### 5.3 Keyword Detection (P0)

- Local keyword detection engine, running in real time on CPU, no GPU required.
- Keywords are freely configured by the user as text; no model training or network connection is needed.
- Supported languages: Mandarin, English (mixed).
- Output keyword hit events (timestamp, matched keyword, confidence); no transcription text is produced.
- Give configuration recommendations for overly short or phonetically common keywords (high false-trigger risk).

### 5.4 Policy Engine and Keyword Triggering (P0)

- Users can configure multiple policies; each policy contains: keyword list, detection threshold, post-hit volume, duration of normal volume, behavior after timeout.
- Policy priority is supported; default action is executed when no policy matches (default mute).
- Detect-and-trigger with sub-second latency.
- Trigger debouncing: repeated hits of the same policy within a short time do not repeat the action, only refresh the duration.
- All trigger events are recorded in a local event log (timestamp, policy name, matched keyword, confidence) for user review; the log can be cleared with one click and is not reported by default.

### 5.5 Volume Control (P0)

- Adjust volume per process without affecting system master volume or other apps.
- Volume transitions are smooth (fade in / fade out) to avoid popping.
- When the app exits or crashes, the target process's original volume is automatically restored.
- Panic shortcut (global, configurable): immediately restore normal volume and pause policy enforcement for a period of time.

### 5.6 User Interface (P1)

- Process selection list + "Start / Stop focus mode" main switch.
- Current status indicators: Muting / Triggered (countdown) / Detecting.
- Recent trigger event notifications.
- Policy editor (form-based, with advanced editing support).
- Event history list.

### 5.7 Unified API (P0)

Expose a unified local API with consistent semantics across platforms; the GUI itself is built on this API (API-first). Capabilities must cover:

- Enumerate capturable audio processes;
- Start / end a "focus session" (specifying process + policy);
- Subscribe to trigger event stream;
- Manually set process volume;
- Query session status;
- Load / validate policy configuration.

The API is only exposed to the local machine; abuse by other local processes must be prevented.

## 6. Non-Functional Requirements

| Category | Requirement |
|----------|-------------|
| Latency | Keyword spoken → volume restored, end-to-end ≤ 1s |
| Hardware | Runs on ordinary CPUs, no GPU required |
| Effectiveness | ≥ 95% detection rate for configured keywords (quiet office environment, Mandarin); false triggers mitigated by debounce / policy fallback |
| Stability | No obvious memory leaks after 8 hours of continuous operation; automatically restores target process volume on crash |
| Compatibility | Mainstream desktop versions of Windows / macOS / Linux from the last 3 years |
| Privacy | Audio processed entirely locally; no transcription text produced; no network requests by default; event log stored locally only |
| Usability | Provides installers / binaries for all three platforms, ready to use out of the box |

## 7. Boundary and Exception Scenarios

- Target process exits / restarts: session automatically terminates, original volume is restored, user is notified.
- Target process has no audio output: show "No audio" status without erroring.
- High system load causing detection lag: notify that latency has increased; on sustained anomaly keep normal volume (fail-safe: better audible than missed).
- User manually adjusts the process volume in the system: app pauses automatic control and notifies; user decides whether to resume control.
- Missing permissions: provide guidance and fail gracefully.
- Multiple meeting processes: MVP supports only a single session; multi-session is a future iteration.

## 8. Metrics and Telemetry (all local, no reporting)

- Daily active sessions, average session duration.
- Triggers per hour, false-trigger feedback (user manual flagging).
- End-to-end trigger latency distribution (p50 / p95).
- Crash rate, volume restore success rate.

## 9. Milestones (Proposed)

| Phase | Content | Acceptance |
|-------|---------|------------|
| M1 Prototype | Single-platform core pipeline: select process → keyword detection → trigger volume | Tencent Meeting / Feishu end-to-end trigger ≤ 1s |
| M2 Cross-platform | Unified API available on all three platforms, policy engine refined | Consistent API behavior across platforms |
| M3 Productization | GUI, policy editor, event log, panic shortcut, installers | US-1 ~ US-7 all accepted |

## 10. Open Questions (Product Level)

1. Should "trigger only on a specific person's voice (e.g., the boss)" be a v2 direction?
2. Besides local access, is there demand for remote control (e.g., mobile phone remote mute / unmute)?
3. Product naming and external positioning: keep positioning as "personal attention management / meeting listening aid tool", avoiding interpretation as encouraging deception of the employer.

## 11. Compliance Statement

- The product only processes meeting audio from meetings the local user is participating in. By default it does not save audio, perform full transcription, or upload data.
- A usage statement is shown on first launch, leaving compliance responsibility and control to the user.
- Some companies / platforms prohibit audio processing of meeting content; users must confirm the policy of their own environment.
