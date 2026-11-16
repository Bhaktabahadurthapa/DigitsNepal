//! Offline, deterministic task understanding for Devanagari Nepali,
//! Romanized Nepali, English and mixed text.
//!
//! It extracts a task category, the actions asked for (and which of them are
//! sensitive), dates (relative words such as "bholi" / "भोलि" normalised to
//! ISO dates), times, money amounts ("5 hajar", "रु ५,०००", "2 lakh", "$50"),
//! Nepali places and preferences. The model refines this into a plan, but the
//! extracted amounts, dates and places are kept as the reference so they are
//! never silently changed (see `tasks::check_plan`).

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DateHint {
    pub text: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Amount {
    pub text: String,
    pub amount: f64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentHints {
    pub language: String,
    pub category: String,
    pub confidence: f32,
    pub alternatives: Vec<String>,
    pub actions: Vec<String>,
    pub sensitive_actions: Vec<String>,
    pub dates: Vec<DateHint>,
    pub times: Vec<String>,
    pub amounts: Vec<Amount>,
    pub locations: Vec<String>,
    pub preferences: Vec<String>,
}

/// (category, strong keywords, weak keywords). Keywords match a word prefix,
/// so Nepali suffixes (ko, ma, le, lai, haru, को, मा …) are tolerated.
const CATEGORIES: &[(&str, &[&str], &[&str])] = &[
    (
        "job_search",
        &[
            "jagir",
            "jaagir",
            "job",
            "rojgar",
            "vacancy",
            "vacancies",
            "cv",
            "resume",
            "biodata",
            "aavedan",
            "awedan",
            "naukari",
            "nokari",
            "जागिर",
            "रोजगार",
            "आवेदन",
            "बायोडाटा",
            "नोकरी",
            "सीभी",
            "सिभी",
        ],
        &["apply", "application", "career", "salary", "talab", "तलब"],
    ),
    (
        "supplier_sourcing",
        &[
            "supplier",
            "wholesale",
            "wholesaler",
            "packaging",
            "bulk",
            "quotation",
            "distributor",
            "thok",
            "aapurti",
            "आपूर्ति",
            "होलसेल",
            "थोक",
            "प्याकेजिङ",
        ],
        &["deal", "order", "moq"],
    ),
    (
        "local_service",
        &[
            "safa",
            "saphai",
            "sapha",
            "cleaning",
            "cleaner",
            "plumber",
            "electrician",
            "mistri",
            "repair",
            "marmat",
            "painter",
            "mover",
            "सफा",
            "सफाई",
            "मर्मत",
            "मिस्त्री",
            "प्लम्बर",
            "सेवा",
        ],
        &["service", "sewa", "ghar", "book", "booking"],
    ),
    (
        "appointment",
        &[
            "appointment",
            "doctor",
            "daktar",
            "clinic",
            "hospital",
            "aspatal",
            "dentist",
            "dental",
            "checkup",
            "salon",
            "डाक्टर",
            "अपोइन्टमेन्ट",
            "अस्पताल",
            "क्लिनिक",
        ],
        &["book", "booking"],
    ),
    (
        "travel",
        &[
            "hotel",
            "flight",
            "bus",
            "ticket",
            "travel",
            "trip",
            "ghumna",
            "yatra",
            "tour",
            "trek",
            "होटल",
            "यात्रा",
            "टिकट",
            "घुम्न",
            "उडान",
        ],
        &["booking", "book", "room"],
    ),
    (
        "shopping",
        &[
            "price",
            "mol",
            "mulya",
            "buy",
            "kinna",
            "kinnu",
            "kinera",
            "product",
            "saman",
            "discount",
            "मूल्य",
            "किन्न",
            "किनेर",
            "सामान",
            "मोल",
        ],
        &["deal", "sasto", "सस्तो", "cheap", "order"],
    ),
    (
        "communication",
        &[
            "email",
            "mail",
            "inbox",
            "reply",
            "chithi",
            "sandesh",
            "jawaf",
            "whatsapp",
            "viber",
            "इमेल",
            "सन्देश",
            "जवाफ",
            "चिठी",
        ],
        &["message", "msg", "draft"],
    ),
    (
        "scheduling",
        &[
            "calendar",
            "meeting",
            "schedule",
            "reminder",
            "samjhana",
            "baithak",
            "बैठक",
            "सम्झना",
            "क्यालेन्डर",
        ],
        &["bhet", "भेट"],
    ),
    (
        "documents",
        &[
            "pdf",
            "document",
            "file",
            "report",
            "summarize",
            "summary",
            "saransh",
            "kagajat",
            "सारांश",
            "कागजात",
            "फाइल",
            "रिपोर्ट",
        ],
        &["folder", "notes"],
    ),
    (
        "translation",
        &["translate", "translation", "anuwad", "anubad", "अनुवाद"],
        &[],
    ),
    (
        "research",
        &[
            "research",
            "compare",
            "tulana",
            "jankari",
            "information",
            "review",
            "तुलना",
            "जानकारी",
            "अनुसन्धान",
        ],
        &["khoj", "search", "find", "खोज"],
    ),
];

/// (action, keywords, sensitive?)
const ACTIONS: &[(&str, &[&str], bool)] = &[
    ("search", &["khoj", "search", "find", "look", "खोज", "पत्ता"], false),
    ("compare", &["compare", "tulana", "best", "तुलना"], false),
    (
        "draft",
        &["draft", "lekh", "prepare", "tayar", "ready", "लेख", "तयार"],
        false,
    ),
    ("summarize", &["summarize", "summary", "saransh", "सारांश"], false),
    ("book", &["book", "booking", "बुक", "reserve"], true),
    (
        "buy",
        &["buy", "kinna", "kinnu", "kinera", "purchase", "order", "किन्न", "किनेर"],
        true,
    ),
    ("pay", &["pay", "payment", "tirna", "tirnu", "भुक्तानी", "तिर्न"], true),
    ("send", &["send", "pathau", "pathaideu", "पठाउ", "पठाइ"], true),
    ("apply", &["apply", "submit", "पेश"], true),
    (
        "negotiate",
        &["negotiate", "molmolai", "bargain", "मोलमोलाई", "arrange"],
        true,
    ),
];

const PLACES: &[(&str, &[&str])] = &[
    ("Kathmandu", &["kathmandu", "ktm", "काठमाडौं", "काठमाडौँ", "काठमाण्डौ"]),
    ("Lalitpur", &["lalitpur", "patan", "ललितपुर", "पाटन"]),
    ("Bhaktapur", &["bhaktapur", "भक्तपुर"]),
    ("Pokhara", &["pokhara", "पोखरा"]),
    ("Chitwan", &["chitwan", "chitawan", "bharatpur", "चितवन", "भरतपुर"]),
    ("Biratnagar", &["biratnagar", "विराटनगर"]),
    ("Butwal", &["butwal", "बुटवल"]),
    ("Dharan", &["dharan", "धरान"]),
    ("Birgunj", &["birgunj", "वीरगञ्ज", "बिरगन्ज"]),
    ("Hetauda", &["hetauda", "हेटौंडा"]),
    ("Janakpur", &["janakpur", "जनकपुर"]),
    ("Nepalgunj", &["nepalgunj", "नेपालगञ्ज"]),
    ("Dhangadhi", &["dhangadhi", "धनगढी"]),
    ("Itahari", &["itahari", "इटहरी"]),
    ("Kirtipur", &["kirtipur", "कीर्तिपुर"]),
    ("Nepal", &["nepal", "नेपाल"]),
];

const PREFERENCES: &[(&str, &[&str])] = &[
    (
        "low_cost",
        &["sasto", "saasto", "cheap", "affordable", "budget", "सस्तो", "किफायती"],
    ),
    (
        "well_reviewed",
        &[
            "review",
            "rating",
            "ramro",
            "best",
            "quality",
            "trusted",
            "राम्रो",
            "विश्वसनीय",
        ],
    ),
    ("urgent", &["urgent", "turunt", "chito", "asap", "तुरुन्त", "छिटो"]),
    ("nearby", &["najik", "nearby", "near", "नजिक"]),
];

fn devanagari_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '०'..='९' => char::from_digit(c as u32 - '०' as u32, 10).unwrap(),
            _ => c,
        })
        .collect()
}

fn tokens(text: &str) -> Vec<String> {
    // Keep thousands separators inside numbers ("5,000" → "5000").
    let lower: Vec<char> = devanagari_digits(&text.to_lowercase()).chars().collect();
    let mut joined = String::with_capacity(lower.len());
    for (i, c) in lower.iter().enumerate() {
        let digit = |j: usize| lower.get(j).is_some_and(|x| x.is_ascii_digit());
        if *c == ',' && i > 0 && digit(i - 1) && digit(i + 1) {
            continue;
        }
        joined.push(*c);
    }
    joined
        .split(|c: char| c.is_whitespace() || ",;:!?()[]{}\"'।॥/".contains(c))
        .map(|t| t.trim_end_matches('.').to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn matches(tok: &str, kw: &str) -> bool {
    // Short Latin keywords must match exactly ("cv", "bus", "mol") to avoid
    // false prefixes; longer ones and Devanagari match as word prefixes.
    if kw.is_ascii() && kw.len() <= 3 {
        tok == kw || tok.strip_suffix('s') == Some(kw)
    } else {
        tok.starts_with(kw)
    }
}

fn any(toks: &[String], kws: &[&str]) -> usize {
    toks.iter().filter(|t| kws.iter().any(|k| matches(t, k))).count()
}

fn parse_number(s: &str) -> Option<f64> {
    let clean: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    if clean.is_empty() || !s.chars().next()?.is_ascii_digit() {
        return None;
    }
    clean.parse().ok()
}

fn multiplier(word: &str) -> Option<f64> {
    match word {
        w if w.starts_with("hajar")
            || w.starts_with("hazar")
            || w.starts_with("हजार")
            || w == "k"
            || w == "thousand" =>
        {
            Some(1e3)
        }
        w if w.starts_with("lakh") || w.starts_with("lac") || w.starts_with("लाख") => Some(1e5),
        w if w.starts_with("crore") || w.starts_with("karod") || w.starts_with("करोड") => Some(1e7),
        _ => None,
    }
}

fn currency_word(word: &str) -> Option<&'static str> {
    match word {
        "rs" | "npr" | "रु" | "रू" | "rupees" | "rupee" => Some("NPR"),
        w if w.starts_with("rupaiy") || w.starts_with("rupiya") || w.starts_with("रुपैयाँ") || w.starts_with("रुपैयां") => {
            Some("NPR")
        }
        "usd" | "dollar" | "dollars" | "$" => Some("USD"),
        "inr" => Some("INR"),
        _ => None,
    }
}

fn amounts(toks: &[String]) -> Vec<Amount> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        // "$50", "rs5000", "रु५०००"
        let (prefix_cur, rest) = if let Some(r) = t.strip_prefix('$') {
            (Some("USD"), r.to_string())
        } else if let Some(r) = t
            .strip_prefix("rs")
            .filter(|r| r.starts_with(|c: char| c.is_ascii_digit() || c == '.'))
        {
            (Some("NPR"), r.trim_start_matches('.').to_string())
        } else if let Some(r) = t.strip_prefix("रु").filter(|r| !r.is_empty()) {
            (Some("NPR"), r.trim_start_matches('.').to_string())
        } else {
            (None, t.clone())
        };
        let mut cur = prefix_cur.map(str::to_string);
        let mut start = i;
        let mut num = parse_number(&rest);
        if num.is_none() && currency_word(t).is_some() {
            if let Some(n) = toks.get(i + 1).and_then(|n| parse_number(n)) {
                cur = currency_word(t).map(str::to_string);
                num = Some(n);
                i += 1;
            }
        }
        if let Some(mut n) = num {
            let mut end = i;
            if let Some(m) = toks.get(i + 1).and_then(|w| multiplier(w)) {
                n *= m;
                end = i + 1;
            }
            if let Some(c) = toks.get(end + 1).and_then(|w| currency_word(w)) {
                cur.get_or_insert(c.to_string());
                end += 1;
            }
            // A bare number is money only when a currency or a multiplier was given.
            if cur.is_some() || end > i {
                if prefix_cur.is_none() && currency_word(&toks[start]).is_none() && start > 0 {
                    if let Some(c) = currency_word(&toks[start - 1]) {
                        cur.get_or_insert(c.to_string());
                        start -= 1;
                    }
                }
                out.push(Amount {
                    text: toks[start..=end].join(" "),
                    amount: n,
                    currency: cur.unwrap_or_else(|| "NPR".into()),
                });
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn weekday(t: &str) -> Option<Weekday> {
    let pairs: &[(&[&str], Weekday)] = &[
        (&["sunday", "aaitabar", "aitabar", "आइतबार"], Weekday::Sun),
        (&["monday", "sombar", "सोमबार"], Weekday::Mon),
        (&["tuesday", "mangalbar", "मंगलबार", "मङ्गलबार"], Weekday::Tue),
        (&["wednesday", "budhabar", "budhbar", "बुधबार"], Weekday::Wed),
        (&["thursday", "bihibar", "बिहीबार", "बिहिबार"], Weekday::Thu),
        (&["friday", "sukrabar", "shukrabar", "शुक्रबार"], Weekday::Fri),
        (&["saturday", "sanibar", "shanibar", "शनिबार"], Weekday::Sat),
    ];
    pairs
        .iter()
        .find(|(ws, _)| ws.iter().any(|w| t.starts_with(w)))
        .map(|(_, d)| *d)
}

fn dates(toks: &[String], today: NaiveDate) -> Vec<DateHint> {
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let rel = match t.as_str() {
            x if ["aaja", "aja", "today", "आज"]
                .iter()
                .any(|w| x == *w || x.starts_with(&format!("{w}ko"))) =>
            {
                Some(0)
            }
            x if ["bholi", "voli", "bholy", "tomorrow", "भोलि", "भोली"]
                .iter()
                .any(|w| x.starts_with(w)) =>
            {
                Some(1)
            }
            x if ["parsi", "parshi", "पर्सि", "पर्सी"].iter().any(|w| x.starts_with(w)) => Some(2),
            x if (x == "hapta" || x == "week" || x == "हप्ता")
                && i > 0
                && ["arko", "next", "अर्को"].contains(&toks[i - 1].as_str()) =>
            {
                Some(7)
            }
            _ => None,
        };
        if let Some(d) = rel {
            let text = if d == 7 {
                format!("{} {t}", toks[i - 1])
            } else {
                t.clone()
            };
            out.push(DateHint {
                text,
                date: (today + Duration::days(d)).to_string(),
            });
        } else if let Some(wd) = weekday(t) {
            let mut diff = (wd.num_days_from_monday() as i64) - (today.weekday().num_days_from_monday() as i64);
            if diff <= 0 {
                diff += 7;
            }
            out.push(DateHint {
                text: t.clone(),
                date: (today + Duration::days(diff)).to_string(),
            });
        } else if let Ok(d) = NaiveDate::parse_from_str(t, "%Y-%m-%d") {
            out.push(DateHint {
                text: t.clone(),
                date: d.to_string(),
            });
        }
    }
    out
}

fn times(toks: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let next = toks.get(i + 1).map(String::as_str).unwrap_or("");
        let hour = |s: &str| s.parse::<u32>().ok().filter(|h| (1..=24).contains(h));
        if ["baje", "बजे", "o'clock"].iter().any(|w| next.starts_with(w)) {
            if let Some(h) = hour(t) {
                out.push(format!("{h:02}:00"));
            }
        } else if let Some(h) = t
            .strip_suffix("pm")
            .and_then(hour)
            .or_else(|| (next == "pm").then(|| hour(t)).flatten())
        {
            out.push(format!("{:02}:00", if h < 12 { h + 12 } else { h }));
        } else if let Some(h) = t
            .strip_suffix("am")
            .and_then(hour)
            .or_else(|| (next == "am").then(|| hour(t)).flatten())
        {
            out.push(format!("{h:02}:00"));
        } else {
            let part = match t.as_str() {
                x if x.starts_with("bihana") || x.starts_with("बिहान") || x == "morning" => Some("morning"),
                x if x.starts_with("diuso") || x.starts_with("दिउँसो") || x == "afternoon" => {
                    Some("afternoon")
                }
                x if x.starts_with("beluka") || x.starts_with("बेलुका") || x == "evening" => {
                    Some("evening")
                }
                x if x.starts_with("rati") || x.starts_with("राति") || x == "night" => Some("night"),
                _ => None,
            };
            if let Some(p) = part {
                out.push(p.to_string());
            }
        }
    }
    out.dedup();
    out
}

pub fn analyze(text: &str) -> IntentHints {
    analyze_at(text, chrono::Local::now().date_naive())
}

pub fn analyze_at(text: &str, today: NaiveDate) -> IntentHints {
    let toks = tokens(text);
    let mut scores: Vec<(&str, f32)> = CATEGORIES
        .iter()
        .map(|(c, strong, weak)| (*c, 3.0 * any(&toks, strong) as f32 + any(&toks, weak) as f32))
        .filter(|(_, s)| *s > 0.0)
        .collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let total: f32 = scores.iter().map(|s| s.1).sum();
    let (category, confidence) = match scores.first() {
        Some((c, s)) if *s >= 2.0 => (c.to_string(), (s / total.max(1.0)).min(1.0)),
        _ => ("general".to_string(), 0.0),
    };
    let alternatives = scores.iter().skip(1).take(2).map(|s| s.0.to_string()).collect();
    let mut actions = Vec::new();
    let mut sensitive = Vec::new();
    for (a, kws, s) in ACTIONS {
        if any(&toks, kws) > 0 {
            actions.push(a.to_string());
            if *s {
                sensitive.push(a.to_string());
            }
        }
    }
    let mut locations: Vec<String> = PLACES
        .iter()
        .filter(|(_, kws)| any(&toks, kws) > 0)
        .map(|(p, _)| p.to_string())
        .collect();
    if locations.len() > 1 {
        locations.retain(|l| l != "Nepal");
    }
    let preferences = PREFERENCES
        .iter()
        .filter(|(_, kws)| any(&toks, kws) > 0)
        .map(|(p, _)| p.to_string())
        .collect();
    IntentHints {
        language: serde_json::to_value(crate::translation::detect::detect(text).language)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default(),
        category,
        confidence: (confidence * 100.0).round() / 100.0,
        alternatives,
        actions,
        sensitive_actions: sensitive,
        dates: dates(&toks, today),
        times: times(&toks),
        amounts: amounts(&toks),
        locations,
        preferences,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 16).unwrap() // a Monday
    }

    #[test]
    fn the_same_job_request_in_three_languages_maps_to_one_intent() {
        for t in [
            "मेरो लागि राम्रो जागिर खोजेर आवेदन तयार गरिदिनुहोस्।",
            "Mero lagi ramro jagir khojera application prepare gardinu.",
            "Find jobs matching my qualifications and prepare applications.",
        ] {
            let h = analyze_at(t, day());
            assert_eq!(h.category, "job_search", "{t}: {h:?}");
            assert!(
                h.actions.contains(&"search".to_string()) || h.actions.contains(&"draft".to_string()),
                "{t}: {h:?}"
            );
        }
    }

    #[test]
    fn everyday_requests_are_categorised() {
        let cases = [
            ("Malai bholi doctor appointment book gardinu.", "appointment"),
            ("Yo product ko best price khoja.", "shopping"),
            (
                "Mero email check garera important message ko reply draft gara.",
                "communication",
            ),
            ("Kathmandu ma sasto hotel khojera booking ko lagi ready gara.", "travel"),
            (
                "Malai bholi ghar safa garne service chahiyo. Ramro review bhako ra sasto khojera book gardinu.",
                "local_service",
            ),
            (
                "Malai restaurant ko lagi affordable packaging supplier khojera best deal arrange gardinu.",
                "supplier_sourcing",
            ),
            ("mero downloads folder ma pdf haru khojera summarize gara", "documents"),
            ("भोलि बिहान १० बजे डाक्टरसँग अपोइन्टमेन्ट चाहियो", "appointment"),
        ];
        for (t, want) in cases {
            assert_eq!(analyze_at(t, day()).category, want, "{t}");
        }
    }

    #[test]
    fn dates_times_and_sensitive_actions() {
        let h = analyze_at("Malai bholi bihana 10 baje doctor appointment book gardinu", day());
        assert_eq!(
            h.dates,
            vec![DateHint {
                text: "bholi".into(),
                date: "2026-11-17".into()
            }]
        );
        assert_eq!(h.times, vec!["morning".to_string(), "10:00".to_string()]);
        assert!(h.sensitive_actions.contains(&"book".to_string()));
        let d = analyze_at("भोलि वा पर्सि, अथवा शुक्रबार", day());
        let got: Vec<_> = d.dates.iter().map(|x| x.date.as_str()).collect();
        assert_eq!(got, vec!["2026-11-17", "2026-11-18", "2026-11-20"]);
        assert_eq!(analyze_at("arko hapta meeting", day()).dates[0].date, "2026-11-23");
        assert_eq!(
            analyze_at("sombar ko meeting", day()).dates[0].date,
            "2026-11-23",
            "next Monday, not today"
        );
    }

    #[test]
    fn money_in_every_common_form() {
        let a = |t: &str| {
            analyze_at(t, day())
                .amounts
                .iter()
                .map(|x| (x.amount, x.currency.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(a("budget 5 hajar samma"), vec![(5000.0, "NPR".into())]);
        assert_eq!(a("रु ५,००० भित्र"), vec![(5000.0, "NPR".into())]);
        assert_eq!(a("Rs. 1,500 per hour"), vec![(1500.0, "NPR".into())]);
        assert_eq!(a("2 lakh ko loan"), vec![(200000.0, "NPR".into())]);
        assert_eq!(a("under $50"), vec![(50.0, "USD".into())]);
        assert_eq!(a("rs5000"), vec![(5000.0, "NPR".into())]);
        assert_eq!(a("3000 rupaiya"), vec![(3000.0, "NPR".into())]);
        assert!(a("mero ghar number 12 ho").is_empty(), "bare numbers are not money");
    }

    #[test]
    fn places_and_preferences() {
        let h = analyze_at("Kathmandu ma sasto ra ramro review bhako hotel", day());
        assert_eq!(h.locations, vec!["Kathmandu".to_string()]);
        assert!(
            h.preferences.contains(&"low_cost".to_string()) && h.preferences.contains(&"well_reviewed".to_string())
        );
        assert_eq!(analyze_at("पोखरामा होटल", day()).locations, vec!["Pokhara".to_string()]);
        assert_eq!(analyze_at("hello there", day()).category, "general");
    }
}
