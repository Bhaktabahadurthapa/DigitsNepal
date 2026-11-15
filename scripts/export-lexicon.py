"""Export src-tauri/src/translation/lexicon.rs to website/data/lexicon.json
so the website's dictionary uses exactly the app's offline lexicon."""
import json, os, re
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
src = open(os.path.join(ROOT, "src-tauri/src/translation/lexicon.rs"), encoding="utf-8").read()

TOKEN = re.compile(r'//\s*(?P<cat>[^\n]+)|e!\(\s*\[(?P<roman>.*?)\],\s*"(?P<deva>.*?)",\s*"(?P<gloss>.*?)"(?P<rest>[^)]*)\)', re.S)

def entries(block):
    out, cat = [], "Phrases"
    for m in TOKEN.finditer(block):
        if m.group("cat"):
            cat = m.group("cat").strip(); continue
        out.append({"roman": re.findall(r'"(.*?)"', m.group("roman")), "deva": m.group("deva"),
                    "gloss": m.group("gloss"), "ambiguous": "ambiguous" in m.group("rest"), "cat": cat})
    return out

phr = src[src.index("pub static PHRASES"):src.index("pub static WORDS")]
words = src[src.index("pub static WORDS"):src.index("pub static SUFFIXES")]
data = {"phrases": entries(phr), "words": entries(words)}
for p in data["phrases"]: p["cat"] = "Phrases"
out = os.path.join(ROOT, "website/data/lexicon.json")
json.dump(data, open(out, "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
print(len(data["phrases"]), "phrases,", len(data["words"]), "words ->", os.path.getsize(out), "bytes")
print(sorted({w["cat"] for w in data["words"]}))
