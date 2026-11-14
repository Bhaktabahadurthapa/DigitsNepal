//! Optional local translation history (opt-in). Stored only on this Mac,
//! as JSON with 0600 permissions. No remote database, no account.

use crate::translation::types::{TranslationMode, TranslationResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MAX_ENTRIES: usize = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub id: String,
    pub created_at: String,
    pub favorite: bool,
    pub mode: TranslationMode,
    pub original_text: String,
    pub accurate_english: String,
    pub styled_english: Option<String>,
    pub nepali_script: Option<String>,
    pub provider: String,
    pub model: String,
}

impl HistoryEntry {
    pub fn from_result(r: &TranslationResult) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            favorite: false,
            mode: r.mode,
            original_text: r.original_text.clone(),
            accurate_english: r.accurate_english.clone(),
            styled_english: r.styled_english.clone(),
            nepali_script: r.nepali_script.clone(),
            provider: r.provider.clone(),
            model: r.model.clone(),
        }
    }
}

pub struct HistoryStore {
    path: PathBuf,
    entries: Vec<HistoryEntry>,
}

impl HistoryStore {
    pub fn open(dir: &Path) -> Self {
        let path = dir.join("history.json");
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path, entries }
    }

    fn persist(&self) -> std::io::Result<()> {
        super::write_atomic(&self.path, &serde_json::to_vec(&self.entries)?)
    }

    pub fn add(&mut self, e: HistoryEntry) -> std::io::Result<HistoryEntry> {
        self.entries.insert(0, e.clone());
        // Trim oldest non-favorites beyond the cap.
        while self.entries.len() > MAX_ENTRIES {
            match self.entries.iter().rposition(|x| !x.favorite) {
                Some(i) => {
                    self.entries.remove(i);
                }
                None => break,
            }
        }
        self.persist()?;
        Ok(e)
    }

    pub fn search(&self, query: &str, favorites_only: bool) -> Vec<HistoryEntry> {
        let q = query.trim().to_lowercase();
        self.entries
            .iter()
            .filter(|e| !favorites_only || e.favorite)
            .filter(|e| {
                q.is_empty()
                    || e.original_text.to_lowercase().contains(&q)
                    || e.accurate_english.to_lowercase().contains(&q)
                    || e.styled_english.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    || e.nepali_script.as_deref().unwrap_or("").contains(query.trim())
            })
            .cloned()
            .collect()
    }

    pub fn set_favorite(&mut self, id: &str, fav: bool) -> std::io::Result<bool> {
        let Some(e) = self.entries.iter_mut().find(|e| e.id == id) else {
            return Ok(false);
        };
        e.favorite = fav;
        self.persist()?;
        Ok(true)
    }

    pub fn delete(&mut self, id: &str) -> std::io::Result<bool> {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        let changed = self.entries.len() != before;
        if changed {
            self.persist()?;
        }
        Ok(changed)
    }

    /// Removes every entry and the file itself.
    pub fn clear(&mut self) -> std::io::Result<()> {
        self.entries.clear();
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "app": "DigitsNepal",
            "format_version": 1,
            "exported_at": chrono::Utc::now().to_rfc3339(),
            "entries": self.entries,
        }))
        .unwrap_or_else(|_| "{}".into())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(o: &str, e: &str) -> HistoryEntry {
        HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: "2026-01-01T00:00:00Z".into(),
            favorite: false,
            mode: TranslationMode::Accurate,
            original_text: o.into(),
            accurate_english: e.into(),
            styled_english: None,
            nepali_script: None,
            provider: "ollama".into(),
            model: "m".into(),
        }
    }

    #[test]
    fn add_search_favorite_delete_clear() {
        let d = tempfile::tempdir().unwrap();
        let mut h = HistoryStore::open(d.path());
        let a = h
            .add(entry("Yo error kina aayo?", "Why did this error occur?"))
            .unwrap();
        let b = h.add(entry("Malai thaha chaina", "I don't know")).unwrap();
        assert_eq!(h.search("ERROR", false).len(), 1);
        assert_eq!(h.search("", false)[0].id, b.id, "newest first");
        assert!(h.set_favorite(&a.id, true).unwrap());
        assert_eq!(h.search("", true).len(), 1);

        // Persistence across reopen.
        let h2 = HistoryStore::open(d.path());
        assert_eq!(h2.len(), 2);

        assert!(h.delete(&b.id).unwrap());
        assert!(!h.delete("missing").unwrap());
        assert_eq!(HistoryStore::open(d.path()).len(), 1);

        h.clear().unwrap();
        assert!(h.is_empty());
        assert!(!d.path().join("history.json").exists());
        assert!(HistoryStore::open(d.path()).is_empty());
    }

    #[test]
    fn export_contains_entries() {
        let d = tempfile::tempdir().unwrap();
        let mut h = HistoryStore::open(d.path());
        h.add(entry("a", "b")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&h.export_json()).unwrap();
        assert_eq!(v["entries"].as_array().unwrap().len(), 1);
        assert_eq!(v["app"], "DigitsNepal");
    }

    #[cfg(unix)]
    #[test]
    fn file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let mut h = HistoryStore::open(d.path());
        h.add(entry("a", "b")).unwrap();
        let mode = std::fs::metadata(d.path().join("history.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn cap_keeps_favorites() {
        let d = tempfile::tempdir().unwrap();
        let mut h = HistoryStore::open(d.path());
        let mut fav = entry("fav", "fav");
        fav.favorite = true;
        h.add(fav).unwrap();
        for i in 0..MAX_ENTRIES {
            h.entries.insert(0, entry(&i.to_string(), "x"));
        }
        h.add(entry("new", "x")).unwrap();
        assert_eq!(h.len(), MAX_ENTRIES);
        assert!(h.search("fav", true).len() == 1);
    }
}
