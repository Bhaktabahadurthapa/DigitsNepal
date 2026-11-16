//! OS-level sandbox for code the agent writes (`code.run`).
//!
//! Each run gets a fresh work folder and executes under macOS Seatbelt
//! (`/usr/bin/sandbox-exec`) with a deny-by-default profile:
//!
//! - **no network** (including DNS);
//! - **no access to the user's files**: only system libraries, the
//!   interpreter's own install folder and the run's work folder can be read;
//!   only the work folder can be written;
//! - **no system services** (no Mach lookups): it cannot open apps, read the
//!   clipboard or send Apple events;
//! - only the interpreter and standard system tools can be executed;
//! - an empty environment, CPU / file-size / open-file limits, a wall-clock
//!   timeout that kills the whole process group, and capped output.
//!
//! Input files are copied in from granted folders; the original files are
//! never visible to the code. The work folder is deleted after the run.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
const MAX_OUTPUT: usize = 64 * 1024;

pub fn available() -> bool {
    cfg!(target_os = "macos") && Path::new(SANDBOX_EXEC).is_file()
}

/// Locate an interpreter in standard install locations (GUI apps have no
/// shell PATH).
pub fn interpreter(language: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates: Vec<String> = match language {
        "python" => vec![
            "/opt/homebrew/bin/python3".into(),
            "/usr/local/bin/python3".into(),
            "/Library/Frameworks/Python.framework/Versions/Current/bin/python3".into(),
            "/usr/bin/python3".into(),
        ],
        "javascript" => {
            let mut v: Vec<String> = vec!["/opt/homebrew/bin/node".into(), "/usr/local/bin/node".into()];
            // nvm installs: newest version first.
            if let Ok(rd) = std::fs::read_dir(format!("{home}/.nvm/versions/node")) {
                let mut vs: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
                vs.sort();
                for p in vs.into_iter().rev() {
                    v.push(p.join("bin/node").display().to_string());
                }
            }
            v
        }
        "shell" => vec!["/bin/sh".into()],
        _ => vec![],
    };
    candidates
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .and_then(|p| std::fs::canonicalize(p).ok())
}

fn quote(p: &Path) -> String {
    let s = p.display().to_string();
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A safe extra folder to allow for the interpreter, never a broad one.
pub fn install_prefix(interp: &Path) -> Option<PathBuf> {
    let p = interp.parent()?.parent()?;
    let depth = p.components().count();
    let system = [
        "/usr",
        "/bin",
        "/sbin",
        "/System",
        "/Library",
        "/opt",
        "/private",
        "/Users",
        "/Applications",
    ];
    let home = std::env::var("HOME").unwrap_or_default();
    if depth < 4 || system.iter().any(|s| p == Path::new(s)) || (!home.is_empty() && p == Path::new(&home)) {
        return None;
    }
    Some(p.to_path_buf())
}

/// The Seatbelt profile for one run.
pub fn profile(work: &Path, interp: &Path) -> String {
    let w = quote(work);
    // The interpreter's own install folder (e.g. …/Python.framework/Versions/3.12),
    // only when it is a specific, deep folder; system interpreters need nothing extra.
    let prefix = install_prefix(interp).unwrap_or_else(|| interp.to_path_buf());
    format!(
        r#"(version 1)
(deny default)
(deny network*)
(deny mach-lookup)
(allow process-fork)
(allow process-exec (subpath "/bin") (subpath "/usr/bin") (subpath "/Library/Developer/CommandLineTools") (subpath "/Applications/Xcode.app") (subpath {pfx}) (literal {interp}))
(allow signal (target self))
(allow sysctl-read)
(allow file-read-metadata)
(allow file-read*
  (literal "/")
  (subpath "/usr/lib") (subpath "/usr/share") (subpath "/usr/bin") (subpath "/bin")
  (subpath "/System") (subpath "/Library/Frameworks") (subpath "/Library/Developer/CommandLineTools")
  (subpath "/Applications/Xcode.app") (subpath "/opt/homebrew") (subpath "/usr/local")
  (subpath "/private/etc") (subpath "/private/var/db/timezone") (subpath "/private/var/select")
  (subpath "/dev")
  (subpath {pfx})
  (subpath {w}))
(allow file-write* (subpath {w}) (literal "/dev/null"))
(allow file-ioctl (literal "/dev/null"))
"#,
        pfx = quote(&prefix),
        interp = quote(interp),
    )
}

fn clip(b: &[u8]) -> (String, bool) {
    let s = String::from_utf8_lossy(b);
    if s.len() <= MAX_OUTPUT {
        return (s.into_owned(), false);
    }
    let mut end = MAX_OUTPUT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    (s[..end].to_string(), true)
}

pub struct Run<'a> {
    pub language: &'a str,
    pub code: &'a str,
    /// Already-resolved files inside granted folders, copied in read-only.
    pub inputs: Vec<PathBuf>,
    pub timeout: Duration,
    /// Parent folder for per-run work folders.
    pub base: &'a Path,
}

/// Execute code in the sandbox. Errors come back as `{"error": …}`.
pub async fn run(r: Run<'_>) -> Value {
    if !available() {
        return json!({"error": "The macOS sandbox is not available on this Mac."});
    }
    let Some(interp) = interpreter(r.language) else {
        return json!({"error": format!("No {} interpreter was found on this Mac.", r.language)});
    };
    let work = r.base.join(format!("run-{}", uuid::Uuid::new_v4()));
    let result = async {
        std::fs::create_dir_all(work.join("input")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(work.join("output")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(work.join("tmp")).map_err(|e| e.to_string())?;
        let work = std::fs::canonicalize(&work).map_err(|e| e.to_string())?;
        let mut copied = Vec::new();
        for p in &r.inputs {
            let name = p.file_name().ok_or("bad input file name")?;
            let dest = work.join("input").join(name);
            std::fs::copy(p, &dest).map_err(|e| format!("Could not copy {}: {e}", p.display()))?;
            let mut perm = std::fs::metadata(&dest).map_err(|e| e.to_string())?.permissions();
            perm.set_readonly(true);
            std::fs::set_permissions(&dest, perm).ok();
            copied.push(format!("input/{}", name.to_string_lossy()));
        }
        let (file, args): (&str, Vec<String>) = match r.language {
            "python" => ("main.py", vec!["-I".into(), "main.py".into()]),
            "javascript" => ("main.js", vec!["main.js".into()]),
            _ => ("main.sh", vec!["main.sh".into()]),
        };
        std::fs::write(work.join(file), r.code).map_err(|e| e.to_string())?;
        std::fs::write(work.join("sandbox.sb"), profile(&work, &interp)).map_err(|e| e.to_string())?;

        let mut cmd = tokio::process::Command::new(SANDBOX_EXEC);
        cmd.arg("-f")
            .arg(work.join("sandbox.sb"))
            .arg(&interp)
            .args(&args)
            .current_dir(&work)
            .env_clear()
            .env("HOME", &work)
            .env("TMPDIR", work.join("tmp"))
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "en_US.UTF-8")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .process_group(0);
        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(|| {
                let lim = |res, v: libc::rlim_t| {
                    let l = libc::rlimit { rlim_cur: v, rlim_max: v };
                    libc::setrlimit(res, &l);
                };
                lim(libc::RLIMIT_CPU, 60);
                lim(libc::RLIMIT_FSIZE, 50 * 1024 * 1024);
                lim(libc::RLIMIT_NOFILE, 256);
                lim(libc::RLIMIT_CORE, 0);
                Ok(())
            });
        }
        let child = cmd.spawn().map_err(|e| format!("Could not start the sandbox: {e}"))?;
        let pid = child.id();
        let started = std::time::Instant::now();
        let out = match tokio::time::timeout(r.timeout, child.wait_with_output()).await {
            Ok(o) => o.map_err(|e| e.to_string())?,
            Err(_) => {
                if let Some(pid) = pid {
                    unsafe {
                        libc::killpg(pid as i32, libc::SIGKILL);
                    }
                }
                return Err(format!("The code ran longer than {} s and was stopped.", r.timeout.as_secs()));
            }
        };
        // Kill anything the code left running in the background.
        if let Some(pid) = pid {
            unsafe {
                libc::killpg(pid as i32, libc::SIGKILL);
            }
        }
        let (stdout, so) = clip(&out.stdout);
        let (stderr, se) = clip(&out.stderr);
        let mut outputs = Vec::new();
        if let Ok(rd) = std::fs::read_dir(work.join("output")) {
            for e in rd.filter_map(Result::ok).take(10) {
                let p = e.path();
                let Ok(meta) = std::fs::symlink_metadata(&p) else { continue };
                if !meta.is_file() {
                    continue;
                }
                let text = (meta.len() <= MAX_OUTPUT as u64)
                    .then(|| std::fs::read(&p).ok())
                    .flatten()
                    .and_then(|b| String::from_utf8(b).ok());
                outputs.push(json!({"name": e.file_name().to_string_lossy(), "bytes": meta.len(), "text": text}));
            }
        }
        Ok::<Value, String>(json!({
            "exit_code": out.status.code(),
            "stdout": stdout,
            "stderr": stderr,
            "truncated": so || se,
            "output_files": outputs,
            "inputs": copied,
            "seconds": (started.elapsed().as_millis() as f64) / 1000.0,
            "note": "Ran in DigitsNepal's sandbox: no network, no access to your files. To keep an output, save it with files.create or documents.create (asks the user)."
        }))
    }
    .await;
    let _ = std::fs::remove_dir_all(&work);
    result.unwrap_or_else(|e| json!({"error": e}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = std::fs::canonicalize(d.path()).unwrap();
        (d, p)
    }

    async fn sh(code: &str, timeout: u64, base: &Path) -> Value {
        run(Run {
            language: "shell",
            code,
            inputs: vec![],
            timeout: Duration::from_secs(timeout),
            base,
        })
        .await
    }

    #[test]
    fn interpreter_prefix_is_never_broad() {
        assert_eq!(install_prefix(Path::new("/bin/sh")), None);
        assert_eq!(install_prefix(Path::new("/usr/bin/python3")), None);
        assert_eq!(install_prefix(Path::new("/opt/homebrew/bin/node")), None);
        assert_eq!(
            install_prefix(Path::new(
                "/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12"
            )),
            Some(PathBuf::from("/Library/Frameworks/Python.framework/Versions/3.12"))
        );
        let p = profile(Path::new("/private/var/w"), Path::new("/bin/sh"));
        assert!(!p.contains("(subpath \"/\")"), "{p}");
    }

    #[test]
    fn profile_escapes_paths() {
        let p = profile(Path::new("/tmp/a\"b"), Path::new("/bin/sh"));
        assert!(p.contains(r#"(subpath "/tmp/a\"b")"#));
        assert!(p.starts_with("(version 1)\n(deny default)"));
    }

    #[tokio::test]
    async fn code_runs_and_can_write_outputs_only_in_its_folder() {
        if !available() {
            return;
        }
        let (_d, b) = base();
        let v = sh(
            "echo hello; echo result > output/r.txt; cat input/x 2>/dev/null; exit 3",
            20,
            &b,
        )
        .await;
        assert_eq!(v["stdout"], "hello\n", "{v}");
        assert_eq!(v["exit_code"], 3);
        assert_eq!(v["output_files"][0]["text"], "result\n");
        assert_eq!(std::fs::read_dir(&b).unwrap().count(), 0, "work folder removed");
    }

    #[tokio::test]
    async fn the_sandbox_blocks_home_network_and_outside_writes() {
        if !available() {
            return;
        }
        let (_d, b) = base();
        let home = std::env::var("HOME").unwrap();
        let outside = std::env::temp_dir().join(format!("dn-escape-{}", uuid::Uuid::new_v4()));
        let code = format!(
            "ls '{home}/Documents' >/dev/null 2>&1 && echo HOME_READ; \
             cat '{home}/.zshrc' >/dev/null 2>&1 && echo ZSHRC_READ; \
             echo x > '{o}' 2>/dev/null && echo WROTE_OUTSIDE; \
             /usr/bin/curl -s -m 3 http://1.1.1.1 >/dev/null 2>&1 && echo NET; \
             /usr/bin/pbpaste >/dev/null 2>&1 && echo CLIPBOARD; \
             /usr/bin/open -a Calculator >/dev/null 2>&1 && echo OPENED_APP; \
             echo done",
            o = outside.display()
        );
        let v = sh(&code, 30, &b).await;
        assert_eq!(v["stdout"], "done\n", "escape: {v}");
        assert!(!outside.exists());
    }

    #[tokio::test]
    async fn runaway_code_is_stopped_with_its_children() {
        if !available() {
            return;
        }
        let (_d, b) = base();
        let started = std::time::Instant::now();
        let v = sh("(sleep 300 &) ; while true; do :; done", 2, &b).await;
        assert!(v["error"].as_str().unwrap().contains("stopped"), "{v}");
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn python_runs_with_copied_inputs_when_installed() {
        if !available() || interpreter("python").is_none() {
            return;
        }
        let (_d, b) = base();
        let src = tempfile::tempdir().unwrap();
        let f = src.path().join("nums.csv");
        std::fs::write(&f, "a\n1\n2\n3\n").unwrap();
        let v = run(Run {
            language: "python",
            code: "import csv\nrows=list(csv.DictReader(open('input/nums.csv')))\nprint(sum(int(r['a']) for r in rows))\ntry:\n    open('input/nums.csv','a').write('x')\n    print('MODIFIED')\nexcept Exception: pass\n",
            inputs: vec![f.clone()],
            timeout: Duration::from_secs(30),
            base: &b,
        })
        .await;
        assert_eq!(v["stdout"], "6\n", "{v}");
        assert_eq!(
            std::fs::read_to_string(&f).unwrap(),
            "a\n1\n2\n3\n",
            "original untouched"
        );
    }
}
