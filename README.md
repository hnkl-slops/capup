# capup

capup records what is behind a translucent window to an mp4 file.
Place the window over the area you want, press record, and keep using the apps behind it.
The capup window itself never appears in the video.

It runs on macOS and Windows as a menu bar / system tray app.

## Features

- **The frame is the recording area.** Drag and resize the translucent window to choose what to record.
- **The window is left out of the video.** Only what is behind it is recorded.
- **The apps behind stay usable while recording.** Clicks pass through the window, except over the control bar.
- **Hardware H.264 encoding.** No ffmpeg or other external tool is needed.
- **Lives in the menu bar / system tray.** Click the icon to open the window on the current desktop.
- **Configurable save folder.** Recordings go to your Movies / Videos folder until you choose another one.

## Requirements

| OS      | Version                 | Status                    |
| ------- | ----------------------- | ------------------------- |
| macOS   | 13 Ventura or later     | Tested                    |
| Windows | 10 version 2004 or later | Builds, not yet tested    |

On macOS, capup needs the **Screen & System Audio Recording** permission.
Grant it in System Settings › Privacy & Security, then restart capup.

## Install

Download the latest build from [Releases](https://github.com/hnkl-slops/capup/releases/latest).

capup is not signed with an Apple Developer ID or a Windows code signing certificate.
The release notes explain how to open it anyway.

## Usage

1. Click the capup icon in the menu bar or system tray to open the window.
2. Drag the window, and resize it from its edges, to cover the area to record.
3. Press **● REC** to start recording.
4. Press **■ STOP** to stop.
   The status shows where the mp4 was saved.
   Click it to reveal the file.

`⌘⇧R` on macOS, or `Ctrl+Shift+R` on Windows, also starts and stops recording.
The shortcut works while another app is in front.

The window cannot be moved or resized while recording.
The recorded area stays the one you started with.

Right-click the tray icon for the menu.

- **Choose Save Folder…** sets where recordings are saved.
- **Quit capup** quits the app and finishes any running recording.

The × button only hides the window.
capup keeps running in the tray.

## How it works

| Step     | macOS                                  | Windows                                  |
| -------- | -------------------------------------- | ---------------------------------------- |
| Capture  | ScreenCaptureKit                       | Windows Graphics Capture                 |
| Exclude own window | Content filter excluding the window | Window content protection          |
| Encode   | AVAssetWriter (VideoToolbox), H.264    | Media Foundation, H.264                  |

The app is built with [Tauri v2](https://v2.tauri.app/).
The UI is TypeScript and Tailwind CSS.
Capture and encoding run natively in Rust, so raw frames never pass through the webview.

## Development

Prerequisites:

- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js](https://nodejs.org/) and [pnpm](https://pnpm.io/)
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS

Run in development mode:

```bash
pnpm install
pnpm tauri dev
```

Build an installable app:

```bash
pnpm tauri build
```

On macOS, the screen recording permission in development mode is granted to the terminal app that runs `pnpm tauri dev`.

### Releasing

1. Bump `version` in `src-tauri/Cargo.toml`. The app takes its version from there.
2. Commit, then push a tag that matches the version:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

The [Release workflow](.github/workflows/release.yml) builds a universal macOS app and a Windows installer, and publishes them as a GitHub release.
The release text comes from [.github/release-notes.md](.github/release-notes.md).
The workflow stops if the tag does not match the version.

### Project layout

```
src/                      UI (TypeScript, Tailwind CSS)
src-tauri/src/
  lib.rs                  Commands, tray, and window events
  region.rs               The display area covered by the window
  on_screen.rs            Keeps the window within its display
  save_dir.rs             The save folder setting
  recorder/
    mod.rs                Runs a recording on its own thread
    mac.rs                ScreenCaptureKit + AVAssetWriter
    windows.rs            Windows Graphics Capture + Media Foundation
```

## License

[The Unlicense](LICENSE)
