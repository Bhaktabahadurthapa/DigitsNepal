//! `research.summarize` (an extractive, cited source pack) and
//! `documents.create` (structured documents written through the same guarded
//! file writer as `files.create`).

use super::files;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const STOP: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "is", "are", "was", "were", "be", "by", "with", "as",
    "at", "it", "this", "that", "from", "has", "have", "had", "not", "but", "they", "their", "will", "can", "we",
    "our", "you", "your", "i", "he", "she", "his", "her", "its", "which", "who", "what", "when", "where", "how", "all",
    "any", "ra", "ko", "ma", "ho", "ki", "le", "lai", "pani", "chha", "cha",
];

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 2)
        .map(str::to_lowercase)
        .filter(|w| !STOP.contains(&w.as_str()))
        .collect()
}

fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '!' | '?' | '।' | '\n') {
            let s = cur.split_whitespace().collect::<Vec<_>>().join(" ");
            if s.chars().count() >= 25 {
                out.push(s);
            }
            cur.clear();
        }
    }
    let s = cur.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() >= 25 {
        out.push(s);
    }
    out
}

fn read_source(path: &str, roots: &[PathBuf]) -> Result<String, String> {
    let is_pdf = Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    let v = if is_pdf {
        files::read_pdf(path, roots)
    } else {
        files::read_file(path, roots)
    }
    .map_err(|e| e.to_string())?;
    Ok(v["text"].as_str().unwrap_or_default().to_string())
}

/// Build a cited source pack: each source gets an ID (S1, S2, …) and the
/// sentences most relevant to the question are ranked across all sources.
pub fn summarize(args: &Value, roots: &[PathBuf]) -> Value {
    let Some(list) = args["sources"].as_array().filter(|l| !l.is_empty()) else {
        return json!({"error": "Give at least one source (a file path or text)."});
    };
    let question = args["question"].as_str().unwrap_or("");
    let max_points = args["max_points"].as_u64().unwrap_or(8).clamp(1, 20) as usize;

    let mut sources = Vec::new();
    let mut texts = Vec::new();
    for (i, s) in list.iter().take(10).enumerate() {
        let id = format!("S{}", i + 1);
        let (label, text) = if let Some(p) = s["path"].as_str() {
            match read_source(p, roots) {
                Ok(t) => (p.to_string(), t),
                Err(e) => {
                    sources.push(json!({"id": id, "label": p, "error": e}));
                    continue;
                }
            }
        } else if let Some(t) = s["text"].as_str() {
            (
                s["label"].as_str().unwrap_or("Provided text").to_string(),
                t.to_string(),
            )
        } else {
            sources.push(json!({"id": id, "error": "Source needs a path or text."}));
            continue;
        };
        sources.push(json!({"id": id, "label": label, "characters": text.chars().count()}));
        texts.push((id, text));
    }
    if texts.is_empty() {
        return json!({"error": "None of the sources could be read.", "sources": sources});
    }

    // Term weights: frequency across all sources, boosted for question terms.
    let mut freq: HashMap<String, f64> = HashMap::new();
    for (_, t) in &texts {
        for w in words(t) {
            *freq.entry(w).or_default() += 1.0;
        }
    }
    let q: Vec<String> = words(question);
    let mut ranked: Vec<(f64, usize, String, String)> = Vec::new();
    for (id, t) in &texts {
        for (pos, s) in sentences(t).into_iter().enumerate() {
            let ws = words(&s);
            if ws.is_empty() {
                continue;
            }
            let mut score: f64 = ws
                .iter()
                .map(|w| freq.get(w).copied().unwrap_or(0.0).ln_1p())
                .sum::<f64>()
                / (ws.len() as f64).sqrt();
            score += 3.0 * ws.iter().filter(|w| q.contains(w)).count() as f64;
            score += 0.5 / (1.0 + pos as f64); // slight preference for early sentences
            let clipped: String = s.chars().take(400).collect();
            ranked.push((score, pos, id.clone(), clipped));
        }
    }
    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut seen = std::collections::HashSet::new();
    let points: Vec<Value> = ranked
        .into_iter()
        .filter(|r| seen.insert(r.3.clone()))
        .take(max_points)
        .map(|(_, _, id, text)| json!({"source": id, "text": text}))
        .collect();
    let excerpts: Vec<Value> = texts
        .iter()
        .map(|(id, t)| json!({"source": id, "opening": t.chars().take(1200).collect::<String>()}))
        .collect();
    json!({
        "question": question,
        "sources": sources,
        "key_points": points,
        "excerpts": excerpts,
        "untrusted_content": true,
        "note": "Extractive: these are quotes from the sources, not conclusions. Cite them as [S1], [S2]… and do not add facts that are not here."
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render a structured document.
pub fn render(args: &Value) -> Result<String, String> {
    let title = args["title"].as_str().unwrap_or("").trim();
    if title.is_empty() {
        return Err("A document needs a title.".into());
    }
    let format = args["format"].as_str().unwrap_or("markdown");
    let sections: Vec<(String, String)> = args["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            (
                s["heading"].as_str().unwrap_or("").trim().to_string(),
                s["body"].as_str().unwrap_or("").trim().to_string(),
            )
        })
        .collect();
    if sections.is_empty() {
        return Err("A document needs at least one section.".into());
    }
    let sources: Vec<&str> = args["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    Ok(match format {
        "html" => {
            let mut h = format!(
                "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{t}</title>\n<style>body{{font:16px/1.6 -apple-system,system-ui,sans-serif;max-width:46rem;margin:2rem auto;padding:0 1rem;color:#1d1d1f}}h1{{font-size:1.8rem}}h2{{margin-top:2rem}}</style>\n</head>\n<body>\n<h1>{t}</h1>\n",
                t = esc(title)
            );
            for (head, body) in &sections {
                if !head.is_empty() {
                    h.push_str(&format!("<h2>{}</h2>\n", esc(head)));
                }
                for para in body.split("\n\n").filter(|p| !p.trim().is_empty()) {
                    h.push_str(&format!("<p>{}</p>\n", esc(para.trim()).replace('\n', "<br>")));
                }
            }
            if !sources.is_empty() {
                h.push_str("<h2>Sources</h2>\n<ol>\n");
                for s in &sources {
                    h.push_str(&format!("<li>{}</li>\n", esc(s)));
                }
                h.push_str("</ol>\n");
            }
            h.push_str("</body>\n</html>\n");
            h
        }
        "text" => {
            let mut t = format!("{title}\n{}\n\n", "=".repeat(title.chars().count().min(80)));
            for (head, body) in &sections {
                if !head.is_empty() {
                    t.push_str(&format!("{head}\n{}\n", "-".repeat(head.chars().count().min(80))));
                }
                t.push_str(body);
                t.push_str("\n\n");
            }
            if !sources.is_empty() {
                t.push_str("Sources\n-------\n");
                for (i, s) in sources.iter().enumerate() {
                    t.push_str(&format!("{}. {s}\n", i + 1));
                }
            }
            t
        }
        _ => {
            let mut m = format!("# {title}\n\n");
            for (head, body) in &sections {
                if !head.is_empty() {
                    m.push_str(&format!("## {head}\n\n"));
                }
                m.push_str(body);
                m.push_str("\n\n");
            }
            if !sources.is_empty() {
                m.push_str("## Sources\n\n");
                for (i, s) in sources.iter().enumerate() {
                    m.push_str(&format!("{}. {s}\n", i + 1));
                }
            }
            m
        }
    })
}

/// `documents.create`: render and write through the guarded file writer.
pub fn create(args: &Value, roots: &[PathBuf], backups: &Path) -> Value {
    let path = args["path"].as_str().unwrap_or("");
    let want = match args["format"].as_str().unwrap_or("markdown") {
        "html" => &["html", "htm"][..],
        "text" => &["txt"][..],
        _ => &["md", "markdown"][..],
    };
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !want.contains(&ext.as_str()) {
        return json!({"error": format!("For this format the file name should end in .{}.", want[0])});
    }
    match render(args) {
        Ok(content) => {
            files::write_file(path, &content, roots, backups).unwrap_or_else(|e| json!({"error": e.to_string()}))
        }
        Err(e) => json!({"error": e}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_ranks_question_terms_and_cites_sources() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("notes.txt");
        std::fs::write(&f, "The weather was pleasant all week long in Pokhara. The budget for the school project is 40,000 rupees and was approved on Friday. Lunch was served at noon in the hall.").unwrap();
        let roots = vec![d.path().canonicalize().unwrap()];
        let out = summarize(
            &json!({"sources": [{"path": f.canonicalize().unwrap()}, {"text": "The project budget may increase next year according to the principal.", "label": "Email"}], "question": "What is the project budget?", "max_points": 2}),
            &roots,
        );
        let pts = out["key_points"].as_array().unwrap();
        assert_eq!(pts.len(), 2);
        assert!(pts[0]["text"].as_str().unwrap().contains("budget"), "{out}");
        assert_eq!(out["sources"][1]["id"], "S2");
        assert_eq!(out["untrusted_content"], true);
    }

    #[test]
    fn summary_reports_unreadable_sources() {
        let out = summarize(&json!({"sources": [{"path": "/etc/hosts"}]}), &[]);
        assert!(out["error"].is_string());
        assert!(out["sources"][0]["error"].is_string());
    }

    #[test]
    fn documents_render_and_escape() {
        let a = json!({"title": "Report <1>", "format": "html", "sections": [{"heading": "Intro", "body": "a & b\n\nnext"}], "sources": ["S1 notes.txt"]});
        let h = render(&a).unwrap();
        assert!(h.contains("<title>Report &lt;1&gt;</title>"));
        assert!(h.contains("<p>a &amp; b</p>\n<p>next</p>"));
        assert!(h.contains("<li>S1 notes.txt</li>"));
        let m = render(&json!({"title": "T", "sections": [{"heading": "H", "body": "B"}]})).unwrap();
        assert_eq!(m, "# T\n\n## H\n\nB\n\n");
        assert!(render(&json!({"title": "T", "sections": []})).is_err());
    }

    #[test]
    fn create_writes_inside_grant_with_matching_extension() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().canonicalize().unwrap();
        let b = tempfile::tempdir().unwrap();
        let args = |p: &str| json!({"path": root.join(p), "title": "T", "format": "markdown", "sections": [{"heading": "", "body": "x"}]});
        assert!(create(&args("r.txt"), std::slice::from_ref(&root), b.path())["error"].is_string());
        let ok = create(&args("r.md"), std::slice::from_ref(&root), b.path());
        assert!(ok.get("error").is_none(), "{ok}");
        assert_eq!(std::fs::read_to_string(root.join("r.md")).unwrap(), "# T\n\nx\n\n");
    }
}
