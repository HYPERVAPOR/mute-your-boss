# Mute Your Boss

![Banner](docs/assets/banner.jpg)

Mute Your Boss automatically restores the volume of a meeting app when someone in the meeting says a configured keyword.

## What It Is

Mute Your Boss helps you tune out a long-winded meeting without worrying you'll miss your cue.

Pick a few keywords — your name, "are you there?", or whatever means someone needs you — and turn the meeting app volume down. When someone in the meeting says one of those keywords, the tool temporarily restores the volume so you know you're being addressed. Once the moment passes, you can mute again and get back to work.

## Supported Apps

The goal is to support common meeting apps on Windows, macOS, and Linux:

- Tencent Meeting
- Feishu / Lark
- Zoom
- Microsoft Teams
- Other apps that expose a separate audio session

Platform-specific adapters are still being implemented. The current release focuses on the core pipeline and Windows support.

## Usage

🚧 **Pending — under active development.**

The core pipeline and platform-independent logic are in place, but the Tauri GUI, platform-specific audio adapters, and end-user packaging are still being built. A step-by-step usage guide will be added once the first usable release is ready.

## Important Notes

- **Local only.** The server listens on `127.0.0.1` and uses a local authentication token. No audio leaves your machine.
- **Manual override.** If you change the target app's volume yourself while a session is active, automatic control pauses so the tool does not fight you.
- **Panic shortcut.** Press `Ctrl+Alt+M` (configurable) at any time to immediately restore volume and pause policies.
- **Model not included.** The keyword model is downloaded separately and ignored by Git.

## License

MIT
