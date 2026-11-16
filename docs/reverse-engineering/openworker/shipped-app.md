# Shipped OpenWorker Desktop App — REA evidence

Target: `OpenWorker-macos-arm64.dmg` from the official release
(`download.openworker.com/mac`), SHA-256
`3fb2f1731e4cdb87c8b9c30ee1158841f93e1a71c551c2c16395e6b92b1982dc`.
Static analysis only: mounted read-only, never installed or executed.

| Finding | Evidence | Confidence |
|---|---|---|
| DMG inventory: 3,744 entries (2,621 files, 1,122 folders, 1 symlink), no integrity contradictions | `ev_4994f788…c6a7` (inspect_artifact summary; parent `ev_f8dcac33…cbe`) | Observed (REA) |
| Bundle `com.openworker.desktop`, version 0.3.1, executable `openworker-desktop`, macOS 12+ | `ev_26061635…f538` (inspect_plist) | Observed (REA) |
| Privacy strings: microphone (local transcription, not uploaded), Desktop, Documents, Downloads, Photos ("never scans on its own") | `ev_26061635…f538` | Observed (REA) |
| Shell binary is Rust (rustc commit path present), arm64, 18.6 MB; no cargo-auditable dependency section | `ev_e1e506d2…86b3` (inspect_rust_binary) | Observed (REA); dependency list Unknown |
| Engine ships as a PyInstaller sidecar: `Resources/sidecar/openworker-server` + `_internal` (Python 3.12, aisuite, boto3, anyio, uvloop…); `coworker` code is compiled into `base_library.zip` except the sandbox runner source, personas and headless assets | `codesign`/`ls` on the mounted bundle | Observed (static) |
| Sandbox runner source shipped as plain files: `coworker/sandbox/runner_src/{daemon,executor,fsops,protocol,relay}.py` (copied into the sandbox at runtime) | file listing | Observed (static); runtime use Medium (matches source) |
| Shell supervises the server: strings show `openworker-server`, `OPENWORKER_PORT`, `127.0.0.1`, `openworker-server.log`, `caffeinate -i -s`, login-shell environment capture markers | `strings` on `openworker-desktop` | Observed (static), behaviour Medium |
| Tauri plugins linked: updater, autostart, dialog, opener, single-instance | `strings` | Observed (static) |
| Voice input linked into the shell (`whisper` symbols) | `strings` | Observed (static) |
| Signed Developer ID, team `4D4TN54F7T`, hardened runtime, notarized, Gatekeeper accepted | `codesign -dv`, `spctl -a -vv` | Observed |
| Entitlements: `device.audio-input`, `cs.disable-library-validation`; **no App Sandbox** | `codesign -d --entitlements` | Observed |

Unknowns: exact runtime process tree and network endpoints (not executed);
the Python engine version inside `base_library.zip` versus repository `ecb4454`.
