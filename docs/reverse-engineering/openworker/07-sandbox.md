# 15 Sandbox and execution isolation

- **Description**: commands and file tools run behind an OS-enforced wall; the agent loop, model keys and connectors stay outside.
- **Desktop backends** (`coworker/sandbox/providers/`): `seatbelt.py` + `seatbelt_profile.py` (macOS, built in, nothing to install), `openshell*.py` (NVIDIA OpenShell Linux container), `windows.py` (hidden local account), `runner_local.py`.
- **macOS design** (`docs/macos-sandbox.md`): profile rendered from the session's folders; kernel-enforced and inherited by child processes; files limited to session folders plus one private temp folder (home, `.ssh`, `.aws`, browser profiles and OpenWorker's own state cannot be read or listed); network only through a local allow-list proxy (`sandbox/netproxy.py`); secret-named environment variables are not passed in; the wall is verified from inside before the session starts.
- **Shipped app**: the sandbox runner source ships as plain files (`coworker/sandbox/runner_src/*`) for injection into the sandbox (see [shipped-app.md](shipped-app.md)). The app itself is not App-Sandboxed.
- **Harness**: `execution::Backend { Openshell, Off }` (`runtime/src/execution.rs:8`). `direct_macos.rs`: "explicit sandbox-off profile … process-group cleanup is lifecycle management, not containment".
- **Implication for DigitsNepal**: there is no macOS containment in the SDK. Command or untrusted-code tools require a DigitsNepal Seatbelt runner (profile per session folder set, deny-by-default file and network rules, proxy for allowed hosts, no secrets in the environment, in-sandbox self-check). Application path validation alone is not a sandbox.
- **Complexity**: High. **Confidence**: High (design docs and module layout); Seatbelt profile rules themselves not reproduced here.
