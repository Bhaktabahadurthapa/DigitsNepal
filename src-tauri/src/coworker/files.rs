//! Folder-scoped file tools for the coworker.
//!
//! Every path is resolved to its real location and must sit inside a folder
//! the user granted. Writes never destroy data: an existing file is copied to
//! the app's backup area first, and the new content is written atomically.
//! There is deliberately no delete tool.

use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

pub const MAX_READ_BYTES: u64 = 256 * 1024;
pub const MAX_WRITE_BYTES: usize = 512 * 1024;
pub const MAX_LIST: usize = 500;
pub const MAX_SEARCH_RESULTS: usize = 200;
pub const MAX_PDF_CHARS: usize = 100_000;
const SKIP_DIRS: &[&str] = &["node_modules", ".git", "target", ".venv", "__pycache__", "Library"];

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FileError {
    #[error("Use a full path (for example /Users/you/Downloads/report.pdf).")]
    NotAbsolute,
    #[error("That location is outside the folders you granted to DigitsNepal.")]
    OutsideGrant,
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Refused(String),
    #[error("{0}")]
    Io(String),
}

/// Resolve `path` to its real location and require it to be inside a granted
/// root. For a path that does not exist yet (`must_exist = false`), the parent
/// must exist and be inside a root.
pub fn resolve_in_roots(path: &str, roots: &[PathBuf], must_exist: bool) -> Result<PathBuf, FileError> {
    let p = expand_home(path.trim());
    if !p.is_absolute() {
        return Err(FileError::NotAbsolute);
    }
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(FileError::OutsideGrant);
    }
    let real = match std::fs::canonicalize(&p) {
        Ok(r) => r,
        Err(_) if !must_exist => {
            let parent = p.parent().ok_or(FileError::OutsideGrant)?;
            let name = p.file_name().ok_or(FileError::OutsideGrant)?;
            std::fs::canonicalize(parent)
                .map_err(|_| FileError::NotFound(parent.display().to_string()))?
                .join(name)
        }
        Err(_) => return Err(FileError::NotFound(p.display().to_string())),
    };
    let roots: Vec<PathBuf> = roots.iter().filter_map(|r| std::fs::canonicalize(r).ok()).collect();
    if roots.iter().any(|r| real.starts_with(r)) {
        Ok(real)
    } else {
        Err(FileError::OutsideGrant)
    }
}

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

/// Files that commonly hold credentials are never read or written by the agent.
pub fn is_protected(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let protected_names = [
        ".env",
        ".npmrc",
        ".netrc",
        ".pypirc",
        "id_rsa",
        "id_ed25519",
        "credentials",
        ".git-credentials",
    ];
    let protected_ext = [
        "pem",
        "key",
        "p12",
        "pfx",
        "keychain",
        "keychain-db",
        "mobileprovision",
        "kdbx",
    ];
    protected_names
        .iter()
        .any(|n| name == *n || name.starts_with(&format!("{n}.")))
        || path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| protected_ext.contains(&e.to_ascii_lowercase().as_str()))
        || path
            .components()
            .any(|c| matches!(c.as_os_str().to_str(), Some(".ssh" | ".gnupg" | ".aws" | "Keychains")))
}

#[derive(Debug, Serialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: &'static str,
    pub size: u64,
    pub modified: i64,
}

fn entry(path: &Path) -> Option<Entry> {
    let md = std::fs::symlink_metadata(path).ok()?;
    if md.file_type().is_symlink() {
        return None;
    }
    Some(Entry {
        name: path.file_name()?.to_string_lossy().to_string(),
        path: path.display().to_string(),
        kind: if md.is_dir() { "folder" } else { "file" },
        size: if md.is_dir() { 0 } else { md.len() },
        modified: md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    })
}

fn hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

pub fn list_folder(path: &str, roots: &[PathBuf]) -> Result<Value, FileError> {
    let dir = resolve_in_roots(path, roots, true)?;
    if !dir.is_dir() {
        return Err(FileError::Refused("That is a file, not a folder.".into()));
    }
    let mut items: Vec<Entry> = std::fs::read_dir(&dir)
        .map_err(|e| FileError::Io(e.to_string()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| !hidden(p))
        .filter_map(|p| entry(&p))
        .collect();
    items.sort_by_key(|e| std::cmp::Reverse(e.modified));
    let truncated = items.len() > MAX_LIST;
    items.truncate(MAX_LIST);
    Ok(json!({"folder": dir.display().to_string(), "entries": items, "truncated": truncated}))
}

/// Case-insensitive name search under a granted folder. `extension` filters by
/// file type (e.g. "pdf"). Hidden folders and build directories are skipped.
pub fn search_files(folder: &str, query: &str, extension: Option<&str>, roots: &[PathBuf]) -> Result<Value, FileError> {
    let root = resolve_in_roots(folder, roots, true)?;
    let q = query.trim().to_lowercase();
    let ext = extension.map(|e| e.trim_start_matches('.').to_ascii_lowercase());
    let mut found = Vec::new();
    let mut stack = vec![(root.clone(), 0usize)];
    let mut scanned = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            scanned += 1;
            if hidden(&p) || scanned > 50_000 {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                let name = e.file_name().to_string_lossy().to_string();
                if depth < 8 && !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push((p, depth + 1));
                }
                continue;
            }
            let name = e.file_name().to_string_lossy().to_lowercase();
            let ext_ok = ext.as_deref().is_none_or(|x| {
                p.extension()
                    .and_then(|y| y.to_str())
                    .is_some_and(|y| y.eq_ignore_ascii_case(x))
            });
            if ext_ok && (q.is_empty() || name.contains(&q)) {
                if let Some(en) = entry(&p) {
                    found.push(en);
                }
                if found.len() >= MAX_SEARCH_RESULTS {
                    break;
                }
            }
        }
        if found.len() >= MAX_SEARCH_RESULTS {
            break;
        }
    }
    found.sort_by_key(|e| std::cmp::Reverse(e.modified));
    Ok(json!({"folder": root.display().to_string(), "results": found, "truncated": found.len() >= MAX_SEARCH_RESULTS}))
}

pub fn read_file(path: &str, roots: &[PathBuf]) -> Result<Value, FileError> {
    let p = resolve_in_roots(path, roots, true)?;
    if is_protected(&p) {
        return Err(FileError::Refused(
            "DigitsNepal does not read files that usually contain passwords or keys.".into(),
        ));
    }
    let md = std::fs::metadata(&p).map_err(|e| FileError::Io(e.to_string()))?;
    if md.is_dir() {
        return Err(FileError::Refused("That is a folder; use list_folder.".into()));
    }
    if p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        return Err(FileError::Refused("This is a PDF; use read_pdf.".into()));
    }
    let bytes = std::fs::read(&p).map_err(|e| FileError::Io(e.to_string()))?;
    let truncated = bytes.len() as u64 > MAX_READ_BYTES;
    let slice = &bytes[..bytes.len().min(MAX_READ_BYTES as usize)];
    if slice.iter().take(4096).filter(|b| **b == 0).count() > 0 {
        return Err(FileError::Refused("This looks like a binary file, not text.".into()));
    }
    Ok(
        json!({"path": p.display().to_string(), "text": String::from_utf8_lossy(slice), "truncated": truncated, "size": md.len()}),
    )
}

/// Extract PDF text with macOS PDFKit in a separate `osascript` process, so a
/// malformed PDF can never crash DigitsNepal itself.
pub fn read_pdf(path: &str, roots: &[PathBuf]) -> Result<Value, FileError> {
    let p = resolve_in_roots(path, roots, true)?;
    if !p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        return Err(FileError::Refused("This is not a PDF file.".into()));
    }
    const SCRIPT: &str = r#"function run(argv) {
  ObjC.import('PDFKit');
  var doc = $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(argv[0]));
  if (!doc || doc.isNil()) { return "__DN_ERR__ unreadable"; }
  if (doc.isLocked) { return "__DN_ERR__ locked"; }
  var s = doc.string;
  return (s && !s.isNil()) ? ObjC.unwrap(s) : "";
}"#;
    let mut child = std::process::Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT, &p.display().to_string()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| FileError::Io(e.to_string()))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                return Err(FileError::Refused("Reading this PDF took too long.".into()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(FileError::Io(e.to_string())),
        }
    }
    let out = child.wait_with_output().map_err(|e| FileError::Io(e.to_string()))?;
    let text = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    if let Some(reason) = text.strip_prefix("__DN_ERR__ ") {
        return Err(FileError::Refused(format!("Could not read this PDF ({reason}).")));
    }
    let chars = text.chars().count();
    let text: String = text.chars().take(MAX_PDF_CHARS).collect();
    Ok(
        json!({"path": p.display().to_string(), "text": text, "truncated": chars > MAX_PDF_CHARS,
              "note": if chars == 0 { "No text layer: this PDF may be a scanned image." } else { "" }}),
    )
}

/// Write a text file inside a granted folder. An existing file is first copied
/// into `backup_root`, so the change can always be undone.
pub fn write_file(path: &str, content: &str, roots: &[PathBuf], backup_root: &Path) -> Result<Value, FileError> {
    if content.len() > MAX_WRITE_BYTES {
        return Err(FileError::Refused(
            "That content is too large to write in one step.".into(),
        ));
    }
    let p = resolve_in_roots(path, roots, false)?;
    if is_protected(&p) {
        return Err(FileError::Refused(
            "DigitsNepal does not write files that usually contain passwords or keys.".into(),
        ));
    }
    if p.is_dir() {
        return Err(FileError::Refused("That path is a folder.".into()));
    }
    let mut backup = None;
    if p.exists() {
        let stamp = super::ledger::now();
        let dir = backup_root.join(stamp.to_string());
        std::fs::create_dir_all(&dir).map_err(|e| FileError::Io(e.to_string()))?;
        let dest = dir.join(p.file_name().unwrap_or_default());
        std::fs::copy(&p, &dest).map_err(|e| FileError::Io(e.to_string()))?;
        backup = Some(dest.display().to_string());
    }
    let tmp = p.with_file_name(format!(
        ".{}.dn-tmp",
        p.file_name().unwrap_or_default().to_string_lossy()
    ));
    std::fs::write(&tmp, content).map_err(|e| FileError::Io(e.to_string()))?;
    std::fs::rename(&tmp, &p).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        FileError::Io(e.to_string())
    })?;
    Ok(json!({"path": p.display().to_string(), "bytes": content.len(), "backup": backup}))
}

/// Move or rename a file within granted folders. Never overwrites an
/// existing destination and never moves credential files or folders.
pub fn move_file(from: &str, to: &str, roots: &[PathBuf]) -> Result<Value, FileError> {
    let src = resolve_in_roots(from, roots, true)?;
    let dst = resolve_in_roots(to, roots, false)?;
    if is_protected(&src) || is_protected(&dst) {
        return Err(FileError::Refused(
            "DigitsNepal does not move files that usually contain passwords or keys.".into(),
        ));
    }
    if src.is_dir() {
        return Err(FileError::Refused(
            "Moving folders is not supported; move files one by one.".into(),
        ));
    }
    if dst.exists() {
        return Err(FileError::Refused(
            "A file already exists at the destination; choose another name.".into(),
        ));
    }
    std::fs::rename(&src, &dst).map_err(|e| FileError::Io(e.to_string()))?;
    Ok(json!({"from": src.display().to_string(), "to": dst.display().to_string()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Vec<PathBuf>) {
        let d = tempfile::tempdir().unwrap();
        let granted = d.path().join("granted");
        std::fs::create_dir_all(granted.join("sub")).unwrap();
        std::fs::write(granted.join("notes.txt"), "Maile hijo meeting ma kura gare.").unwrap();
        std::fs::write(granted.join("sub/report.PDF"), "%PDF-").unwrap();
        std::fs::write(granted.join(".env"), "API_KEY=x").unwrap();
        std::fs::write(d.path().join("outside.txt"), "private").unwrap();
        let roots = vec![granted];
        (d, roots)
    }

    #[test]
    fn paths_must_stay_inside_grants() {
        let (d, roots) = setup();
        let r = &roots[0];
        assert!(resolve_in_roots(&r.join("notes.txt").display().to_string(), &roots, true).is_ok());
        assert_eq!(
            resolve_in_roots(&d.path().join("outside.txt").display().to_string(), &roots, true),
            Err(FileError::OutsideGrant)
        );
        assert_eq!(
            resolve_in_roots(&format!("{}/../outside.txt", r.display()), &roots, true),
            Err(FileError::OutsideGrant)
        );
        assert_eq!(resolve_in_roots("notes.txt", &roots, true), Err(FileError::NotAbsolute));
        assert_eq!(
            resolve_in_roots(&r.display().to_string(), &[], true),
            Err(FileError::OutsideGrant),
            "no grants, no access"
        );
    }

    #[test]
    fn symlinks_cannot_escape_a_grant() {
        let (d, roots) = setup();
        let link = roots[0].join("escape");
        std::os::unix::fs::symlink(d.path(), &link).unwrap();
        assert_eq!(
            resolve_in_roots(&link.join("outside.txt").display().to_string(), &roots, true),
            Err(FileError::OutsideGrant)
        );
        // Listing never follows or reports the symlink.
        let v = list_folder(&roots[0].display().to_string(), &roots).unwrap();
        assert!(!v.to_string().contains("escape"));
    }

    #[test]
    fn search_finds_by_extension_and_skips_hidden() {
        let (_d, roots) = setup();
        let v = search_files(&roots[0].display().to_string(), "", Some("pdf"), &roots).unwrap();
        let names: Vec<_> = v["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["report.PDF"]);
        let v = search_files(&roots[0].display().to_string(), "env", None, &roots).unwrap();
        assert!(
            v["results"].as_array().unwrap().is_empty(),
            "hidden files are not searched"
        );
    }

    #[test]
    fn credential_files_are_refused() {
        let (_d, roots) = setup();
        let env = roots[0].join(".env").display().to_string();
        assert!(matches!(read_file(&env, &roots), Err(FileError::Refused(_))));
        assert!(is_protected(Path::new("/Users/x/.ssh/config")));
        assert!(is_protected(Path::new("/tmp/cert.PEM")));
        assert!(!is_protected(Path::new("/tmp/notes.md")));
    }

    #[test]
    fn writes_back_up_existing_files_and_never_leave_the_grant() {
        let (d, roots) = setup();
        let backups = d.path().join("backups");
        let notes = roots[0].join("notes.txt");
        let v = write_file(&notes.display().to_string(), "New notes", &roots, &backups).unwrap();
        assert_eq!(std::fs::read_to_string(&notes).unwrap(), "New notes");
        let backup = v["backup"].as_str().unwrap();
        assert_eq!(
            std::fs::read_to_string(backup).unwrap(),
            "Maile hijo meeting ma kura gare."
        );
        // A new file needs no backup.
        let v = write_file(
            &roots[0].join("brief.md").display().to_string(),
            "# Brief",
            &roots,
            &backups,
        )
        .unwrap();
        assert!(v["backup"].is_null());
        // Outside the grant: refused, and nothing is created.
        let outside = d.path().join("evil.txt");
        assert_eq!(
            write_file(&outside.display().to_string(), "x", &roots, &backups).unwrap_err(),
            FileError::OutsideGrant
        );
        assert!(!outside.exists());
    }

    #[test]
    fn move_stays_inside_grants_and_never_overwrites() {
        let (d, roots) = setup();
        let r = &roots[0];
        let a = r.join("notes.txt").display().to_string();
        let b = r.join("sub/notes-moved.txt").display().to_string();
        assert!(move_file(&a, &b, &roots).is_ok());
        assert!(!std::path::Path::new(&a).exists());
        std::fs::write(&a, "again").unwrap();
        assert!(
            matches!(move_file(&a, &b, &roots), Err(FileError::Refused(_))),
            "no overwrite"
        );
        let out = d.path().join("outside-moved.txt").display().to_string();
        assert_eq!(move_file(&a, &out, &roots).unwrap_err(), FileError::OutsideGrant);
        assert!(
            std::path::Path::new(&a).exists(),
            "refused move leaves the file in place"
        );
        let env = r.join(".env").display().to_string();
        assert!(matches!(
            move_file(&env, &r.join("e.txt").display().to_string(), &roots),
            Err(FileError::Refused(_))
        ));
    }

    #[test]
    fn read_pdf_rejects_non_pdfs_and_reports_unreadable_pdfs() {
        let (_d, roots) = setup();
        let notes = roots[0].join("notes.txt").display().to_string();
        assert!(matches!(read_pdf(&notes, &roots), Err(FileError::Refused(_))));
        let fake = roots[0].join("sub/report.PDF").display().to_string();
        assert!(
            read_pdf(&fake, &roots).is_err(),
            "a corrupt PDF is an error, not a crash"
        );
    }
}
