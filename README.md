# Orbi

A tiny Windows 11 tray app. When something connects (earbuds, Wi-Fi, Ethernet, a USB drive), a polished popup
slides up showing a live visual of it, like the AirPods popup on a MacBook.

Goals: near-zero idle CPU and RAM, event-driven (no polling), works for any connection type.
Full plan, architecture and progress log: `Orbi-Tech-Plan.pdf`.

## Status

| Chunk | Scope | State |
| --- | --- | --- |
| 1 | Tauri 2 scaffold, tray-only app | done |
| 2 | Popup on demand, UI layouts, mock events | done |
| 3 | Core (state, dedupe, debounce, low-battery rule), Watcher trait, pipeline | done, 7 unit tests |
| 4 | Bluetooth watcher (generic battery) | built, not verified on hardware |
| 5 | AirPods left / right / case parsing | built, parser unit tested, not verified on hardware |
| 6 | Wi-Fi, Ethernet, USB watchers | done (Wi-Fi read verified live) |
| Design | Orbit design language | done (v1) |
| 7 | Polish (your mockups, Mica, settings, autostart) | not started |
| 8 | Installer and measurements | not started |

## Layout

- `src-tauri/src/lib.rs`: tray and wiring
- `src-tauri/src/events.rs`: normalized `DeviceEvent`
- `src-tauri/src/core.rs`: live-device state and popup rules (unit tested)
- `src-tauri/src/pipeline.rs`: watchers -> core -> popup/tray tooltip
- `src-tauri/src/popup.rs`: popup window, created on demand and destroyed after about 5 s
- `src-tauri/src/watchers/`: `Watcher` trait; real watchers are added here
- `src-tauri/src/mock.rs`: fake devices for development
- `src/`, `index.html`: popup UI (`main.ts` layouts, `art.ts` SVG artwork, `styles.css`; layout chosen by device type)
- `src-tauri/src/watchers/`: `bluetooth.rs`, `airpods.rs`, `network.rs` (Wi-Fi, Ethernet), `usb.rs`
- `prototype-electron/`: old throwaway prototype, to be deleted

## Requirements (Windows 11)

- Rust (stable) and the Visual Studio C++ Build Tools ("Desktop development with C++")
- Node.js and npm
- WebView2 (included with Windows 11)
- Windows Smart App Control must be off, or it blocks unsigned Rust build helpers

## Run

```
npm install
npm run tauri build -- --debug --no-bundle
src-tauri\target\debug\orbi.exe --mock=mock_airpods
```

Mock ids: `mock_earbuds`, `mock_airpods`, `mock_wifi`, `mock_ethernet`, `mock_usb`. The tray menu also has a Mock entry
for each. Use `npm run tauri build`, not a plain `cargo build`: a plain cargo build makes the popup load the dev
server and show a blank error page.

Tests: `cd src-tauri && cargo test` (18 tests).

Known limits: devices already connected at launch do not pop up; USB hard drives and SSDs are not detected (Windows reports them as fixed disks); Bluetooth and AirPods code needs real hardware to verify.
