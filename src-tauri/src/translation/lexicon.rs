//! A small, hand-curated Romanized Nepali lexicon.
//!
//! Used for two things that must work fully offline:
//! 1. Language detection (Romanized Nepali uses Latin script, so script
//!    detection alone cannot distinguish it from English).
//! 2. Dictionary glosses in the Word Meaning Explorer when no model is
//!    available. These are *dictionary* meanings, clearly labelled as such —
//!    contextual meaning comes from the model.

pub struct Entry {
    /// Romanized spellings, lowercase. Several spellings are common.
    pub roman: &'static [&'static str],
    pub devanagari: &'static str,
    pub gloss: &'static str,
    /// Marker strength for language detection (0.0–1.0). Short words that
    /// collide with English or other languages get a lower weight.
    pub weight: f32,
    /// True when the romanization maps to several different Nepali words.
    pub ambiguous: bool,
}

macro_rules! e {
    ([$($r:literal),+], $d:literal, $g:literal) => {
        Entry { roman: &[$($r),+], devanagari: $d, gloss: $g, weight: 1.0, ambiguous: false }
    };
    ([$($r:literal),+], $d:literal, $g:literal, w=$w:literal) => {
        Entry { roman: &[$($r),+], devanagari: $d, gloss: $g, weight: $w, ambiguous: false }
    };
    ([$($r:literal),+], $d:literal, $g:literal, w=$w:literal, ambiguous) => {
        Entry { roman: &[$($r),+], devanagari: $d, gloss: $g, weight: $w, ambiguous: true }
    };
}

/// Multi-word expressions, matched before single words.
pub static PHRASES: &[Entry] = &[
    e!(
        ["ko barema", "ko baare ma", "ko barima"],
        "को बारेमा",
        "about; regarding"
    ),
    e!(["kura garnu"], "कुरा गर्नु", "to talk; to discuss"),
    e!(
        ["kura gareko", "kura gareko thiye"],
        "कुरा गरेको",
        "talked / discussed (perfective)"
    ),
    e!(["thaha chaina", "thaha chhaina"], "थाहा छैन", "(I) don't know"),
    e!(["ke cha", "ke chha"], "के छ", "what's up; how is it going"),
    e!(["kasto cha", "kasto chha"], "कस्तो छ", "how is it"),
    e!(["kina bhane"], "किन भने", "because"),
    e!(["maaf garnus", "maf garnus"], "माफ गर्नुस्", "please excuse me; sorry"),
    e!(["clear bhayena"], "क्लियर भएन", "did not become clear; was not clear"),
    e!(["ready chaina", "ready chhaina"], "रेडी छैन", "is not ready"),
    e!(["ready chainan", "ready chhainan"], "रेडी छैनन्", "are not ready"),
];

pub static WORDS: &[Entry] = &[
    // Pronouns
    e!(["maile"], "मैले", "I (as the doer of an action)"),
    e!(["malai"], "मलाई", "me; to me"),
    e!(["mero"], "मेरो", "my"),
    e!(
        ["ma"],
        "म / मा",
        "'I' (म), or the postposition 'in/at/on' (मा) after a noun",
        w = 0.6,
        ambiguous
    ),
    e!(["hami"], "हामी", "we"),
    e!(["hamile"], "हामीले", "we (as the doer)"),
    e!(["hamro"], "हाम्रो", "our"),
    e!(["hamilai"], "हामीलाई", "us; to us"),
    e!(["timi"], "तिमी", "you (familiar)"),
    e!(["timile"], "तिमीले", "you (familiar, as the doer)"),
    e!(["timro"], "तिम्रो", "your (familiar)"),
    e!(["timilai"], "तिमीलाई", "you; to you (familiar)"),
    e!(["tapai", "tapain", "tapaai"], "तपाईं", "you (respectful)"),
    e!(["tapaile", "tapainle"], "तपाईंले", "you (respectful, as the doer)"),
    e!(["tapaiko", "tapainko"], "तपाईंको", "your (respectful)"),
    e!(["tapailai", "tapainlai"], "तपाईंलाई", "you; to you (respectful)"),
    e!(["usle"], "उसले", "he/she (familiar, as the doer)"),
    e!(["uslai"], "उसलाई", "him/her (familiar)"),
    e!(["uni"], "उनी", "he/she (respectful)"),
    e!(["unle"], "उनले", "he/she (respectful, as the doer)"),
    e!(["yo"], "यो", "this", w = 0.8),
    e!(["tyo"], "त्यो", "that"),
    e!(["yaha", "yahan"], "यहाँ", "here"),
    e!(["tyaha", "tyahan"], "त्यहाँ", "there"),
    // Postpositions / case markers
    e!(
        ["ko"],
        "को",
        "of; 's (possessive) — can also mean 'who'",
        w = 0.7,
        ambiguous
    ),
    e!(["ka"], "का", "of (plural/respectful possessive)", w = 0.4),
    e!(
        ["ki"],
        "की / कि",
        "of (feminine possessive), or 'or/that'",
        w = 0.4,
        ambiguous
    ),
    e!(["le"], "ले", "marks the doer of an action; by", w = 0.6),
    e!(["lai"], "लाई", "to; for (object marker)"),
    e!(["bata"], "बाट", "from; via; by"),
    e!(["sanga", "sang"], "सँग", "with"),
    e!(["samma"], "सम्म", "until; up to"),
    e!(["pachi", "pachhi"], "पछि", "after; later"),
    e!(["agadi"], "अगाडि", "before; in front of"),
    e!(["bhitra"], "भित्र", "inside; within"),
    e!(["lagi"], "लागि", "for"),
    e!(["barema"], "बारेमा", "about"),
    // Conjunctions & particles
    e!(["ra"], "र", "and", w = 0.5),
    e!(["tara"], "तर", "but"),
    e!(["ani"], "अनि", "and then; and"),
    e!(["pani"], "पनि", "also; too; even"),
    e!(["kinabhane"], "किनभने", "because"),
    e!(["bhane"], "भने", "if; (someone) said", w = 0.8, ambiguous),
    e!(["tyesaile", "tesaile"], "त्यसैले", "therefore; so"),
    e!(["nai"], "नै", "indeed; exactly (emphasis)", w = 0.6),
    e!(["ta"], "त", "emphasis particle ('then', 'though')", w = 0.3),
    e!(["ni"], "नि", "emphasis particle ('you know', 'right?')", w = 0.4),
    e!(["re"], "रे", "reportedly (hearsay particle)", w = 0.3),
    e!(["hola"], "होला", "probably; might be"),
    e!(["rahechha", "rahecha"], "रहेछ", "(it) turns out (to be)"),
    e!(["ekdam"], "एकदम", "very; completely"),
    e!(["haru"], "हरू", "plural marker"),
    // Time
    e!(["hijo"], "हिजो", "yesterday"),
    e!(["aaja", "aja", "aju"], "आज", "today"),
    e!(["bholi"], "भोलि", "tomorrow"),
    e!(["ahile", "aile", "ahiley"], "अहिले", "now"),
    e!(["pahile", "pahila"], "पहिले", "before; earlier; first"),
    e!(["sadhai", "sadhain"], "सधैं", "always"),
    e!(["kahile"], "कहिले", "when"),
    e!(["feri", "pheri"], "फेरि", "again"),
    e!(["chadai", "chaadai", "chado"], "चाँडै", "soon; quickly"),
    e!(["hapta"], "हप्ता", "week"),
    e!(["mahina"], "महिना", "month"),
    e!(["barsa", "barsha"], "वर्ष", "year"),
    // Quantity
    e!(["dherai"], "धेरै", "a lot; many; very"),
    e!(["ali"], "अलि", "a little; somewhat", w = 0.5),
    e!(["sabai", "sabbai"], "सबै", "all; everyone"),
    e!(["kehi", "kei"], "केही", "some; anything"),
    e!(["aru"], "अरू", "others; more"),
    e!(["arko"], "अर्को", "another; next"),
    // Questions
    e!(["kina"], "किन", "why"),
    e!(["ke"], "के", "what", w = 0.7),
    e!(["kaha", "kahan"], "कहाँ", "where"),
    e!(["kasari"], "कसरी", "how"),
    e!(["kun"], "कुन", "which"),
    e!(["kati"], "कति", "how much; how many"),
    e!(["kasto"], "कस्तो", "what kind of; how"),
    // Copulas / auxiliaries
    e!(["cha", "chha"], "छ", "is; there is"),
    e!(["chan", "chhan"], "छन्", "are; there are"),
    e!(["chu", "chhu"], "छु", "(I) am"),
    e!(["chaina", "chhaina"], "छैन", "is not; there is not"),
    e!(["chainan", "chhainan"], "छैनन्", "are not; there are not"),
    e!(["ho"], "हो", "is (identity); yes", w = 0.7),
    e!(["hoina", "hoena"], "होइन", "is not (identity); no"),
    e!(["thiyo"], "थियो", "was"),
    e!(
        ["thiye", "thie"],
        "थिएँ / थिए",
        "'I was / I had' (थिएँ), or 'they were' (थिए)",
        w = 1.0,
        ambiguous
    ),
    e!(["thiyena", "thiena"], "थिएन", "was not"),
    e!(["bhayo"], "भयो", "happened; became; is done"),
    e!(["bhayena", "bhaena"], "भएन", "did not happen; did not become"),
    e!(["bhaeko", "bhayeko"], "भएको", "has happened; having been"),
    e!(["huncha", "hunchha"], "हुन्छ", "happens; will be; OK"),
    e!(
        ["hudaina", "hudaina", "hundaina"],
        "हुँदैन",
        "does not happen; will not be"
    ),
    e!(["huna"], "हुन", "to be"),
    e!(["hunu"], "हुनु", "to be; to happen"),
    e!(["garda"], "गर्दा", "while doing; when doing"),
    e!(["herda"], "हेर्दा", "while looking; when checking"),
    e!(
        ["aaucha", "aauchha", "auchha", "aucha"],
        "आउँछ",
        "comes; appears; will come"
    ),
    e!(["aauna", "auna"], "आउन", "to come (in order to)"),
    e!(
        ["bhairacha", "bhairachha", "bhairaheko"],
        "भइरहेछ",
        "keeps happening; is continuously happening"
    ),
    e!(["kata"], "कता", "where; which way"),
    e!(["hidis", "hidyo"], "हिँडिस् / हिँड्यो", "went; left (informal)"),
    e!(["chito", "chhito"], "छिटो", "quickly; fast"),
    e!(["aija", "aaija"], "आइज", "come (very informal command)"),
    e!(["oe", "oi"], "ओइ", "hey (informal call)", w = 0.5),
    e!(["la"], "ल", "okay; well (particle)", w = 0.4),
    e!(["jana"], "जना", "people (counter for persons)", w = 0.6),
    e!(["sayad", "shayad"], "शायद", "maybe; probably"),
    e!(["ajhai", "aajhai", "ahile samma"], "अझै", "still; yet"),
    e!(["matra", "matrai"], "मात्र", "only"),
    // Common verbs
    e!(["garnu"], "गर्नु", "to do"),
    e!(["garna"], "गर्न", "to do (in order to)"),
    e!(
        ["gare"],
        "गरें / गरे",
        "did — 'I did' (गरें) or 'they did' (गरे)",
        w = 1.0,
        ambiguous
    ),
    e!(["garyo"], "गर्‍यो", "did (he/she/it)"),
    e!(["gareko"], "गरेको", "done; having done"),
    e!(["gardai"], "गर्दै", "doing; while doing"),
    e!(["garchu", "garchhu"], "गर्छु", "I do; I will do"),
    e!(["gardina"], "गर्दिन", "I don't do; I won't do"),
    e!(["garnus", "garnuhos"], "गर्नुस्", "please do"),
    e!(["garera"], "गरेर", "having done; by doing"),
    e!(["garne"], "गर्ने", "to be done; one who does; will do"),
    e!(["sakina", "sakina"], "सकिनँ", "I could not"),
    e!(["sakchu", "sakchhu"], "सक्छु", "I can"),
    e!(["sakdina"], "सक्दिन", "I cannot"),
    e!(["sakiyo"], "सकियो", "is finished; could be done"),
    e!(["sakincha", "sakinchha"], "सकिन्छ", "can be done"),
    e!(["pathaunu"], "पठाउनु", "to send"),
    e!(["pathauna"], "पठाउन", "to send (in order to)"),
    e!(["pathaye", "pathae"], "पठाएँ", "I sent"),
    e!(["aayo", "ayo"], "आयो", "came; arrived; appeared"),
    e!(["aaunu", "aunu"], "आउनु", "to come"),
    e!(["janu"], "जानु", "to go"),
    e!(["gaye", "gaen"], "गएँ", "I went"),
    e!(["gayo"], "गयो", "went"),
    e!(["dinu"], "दिनु", "to give"),
    e!(["diyo"], "दियो", "gave"),
    e!(["dinus", "dinuhos"], "दिनुस्", "please give"),
    e!(["linu"], "लिनु", "to take"),
    e!(["khanu"], "खानु", "to eat"),
    e!(["khana"], "खाना", "food; meal"),
    e!(["khayo"], "खायो", "ate"),
    e!(["bujhe", "bujhen"], "बुझें", "I understood"),
    e!(["bujhina"], "बुझिनँ", "I did not understand"),
    e!(["bujhnu"], "बुझ्नु", "to understand"),
    e!(["thaha", "thaa"], "थाहा", "knowledge; (to) know"),
    e!(["paryo"], "पर्‍यो", "had to; it became necessary"),
    e!(["parcha", "parchha"], "पर्छ", "is necessary; have to"),
    e!(["parne"], "पर्ने", "required; should"),
    e!(["chahincha", "chahinchha"], "चाहिन्छ", "is needed"),
    e!(["chahiyo"], "चाहियो", "is needed; want"),
    e!(["milcha", "milchha"], "मिल्छ", "works out; fits; matches"),
    e!(["milena"], "मिलेन", "did not work out; did not match"),
    e!(["kura"], "कुरा", "talk; matter; thing"),
    e!(["bhanyo"], "भन्यो", "said (he/she)"),
    e!(["bhannu"], "भन्नु", "to say"),
    e!(["herna"], "हेर्न", "to look; to check (in order to)"),
    e!(["hernus"], "हेर्नुस्", "please look; please check"),
    e!(["rakhnu"], "राख्नु", "to keep; to put"),
    e!(["chalyo"], "चल्यो", "ran; worked; started running"),
    e!(["chalena"], "चलेन", "did not run; did not work"),
    e!(["bigriyo"], "बिग्रियो", "broke; got spoiled"),
    // Adjectives
    e!(["ramro"], "राम्रो", "good; nice"),
    e!(["naramro"], "नराम्रो", "bad"),
    e!(["thik", "theek"], "ठीक", "okay; right; fine"),
    e!(["sahi"], "सही", "correct"),
    e!(["galat"], "गलत", "wrong"),
    e!(["garo", "gaaro"], "गाह्रो", "difficult"),
    e!(["sajilo"], "सजिलो", "easy"),
    e!(["naya", "nayan"], "नयाँ", "new"),
    e!(["purano"], "पुरानो", "old"),
    e!(["thulo"], "ठूलो", "big"),
    e!(["sano"], "सानो", "small"),
    e!(["spasta", "spashta"], "स्पष्ट", "clear"),
    // Nouns
    e!(["kaam"], "काम", "work; job; task"),
    e!(["ghar"], "घर", "home; house"),
    e!(["manche", "manchhe"], "मान्छे", "person"),
    e!(["sathi", "saathi"], "साथी", "friend"),
    e!(["samasya"], "समस्या", "problem"),
    e!(["samadhan"], "समाधान", "solution"),
    e!(["jankari"], "जानकारी", "information"),
    e!(["prashna", "prasna"], "प्रश्न", "question"),
    e!(["uttar"], "उत्तर", "answer"),
    e!(["din"], "दिन", "day", w = 0.5),
    e!(["bela"], "बेला", "time; moment"),
    e!(["paisa"], "पैसा", "money"),
    e!(["kitab"], "किताब", "book"),
    e!(["guru"], "गुरु", "teacher", w = 0.5),
    // Politeness
    e!(["dhanyabad", "dhanyabaad", "dhanyawad"], "धन्यवाद", "thank you"),
    e!(["namaste", "namaskar"], "नमस्ते", "hello (respectful greeting)"),
    e!(["maaf", "maf"], "माफ", "pardon; forgiveness"),
    e!(["kripaya"], "कृपया", "please"),
    // Numbers
    e!(["ek"], "एक", "one", w = 0.5),
    e!(["dui"], "दुई", "two"),
    e!(["tin"], "तीन", "three", w = 0.3),
];

/// Productive Nepali verb/case endings in Romanized text. Used as a weaker
/// fallback signal for words not in the lexicon.
pub static SUFFIXES: &[&str] = &[
    "eko", "era", "dai", "chha", "chhu", "chhan", "aunu", "nus", "nuhos", "ena", "enan", "dina", "dainan", "lai",
    "bata", "sanga", "haru", "harulai",
];

/// Very common English function words. Used to spot English and mixed text.
pub static ENGLISH_MARKERS: &[&str] = &[
    "the",
    "a",
    "an",
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "am",
    "i",
    "you",
    "he",
    "she",
    "it",
    "we",
    "they",
    "me",
    "him",
    "her",
    "us",
    "them",
    "my",
    "your",
    "his",
    "its",
    "our",
    "their",
    "this",
    "that",
    "these",
    "those",
    "and",
    "but",
    "or",
    "so",
    "because",
    "if",
    "of",
    "to",
    "in",
    "on",
    "at",
    "for",
    "with",
    "from",
    "by",
    "about",
    "as",
    "into",
    "not",
    "no",
    "do",
    "does",
    "did",
    "have",
    "has",
    "had",
    "will",
    "would",
    "can",
    "could",
    "should",
    "may",
    "might",
    "must",
    "what",
    "why",
    "how",
    "when",
    "where",
    "which",
    "who",
    "there",
    "here",
    "then",
    "than",
    "very",
    "just",
    "also",
    "please",
    "thanks",
    "yes",
    "yesterday",
    "today",
    "tomorrow",
    "now",
    "after",
    "before",
    "again",
    "all",
    "some",
];

pub fn lookup_word(token: &str) -> Option<&'static Entry> {
    let t = token.to_lowercase();
    WORDS.iter().find(|e| e.roman.contains(&t.as_str()))
}

pub fn is_english_marker(token: &str) -> bool {
    let t = token.to_lowercase();
    ENGLISH_MARKERS.contains(&t.as_str())
}
