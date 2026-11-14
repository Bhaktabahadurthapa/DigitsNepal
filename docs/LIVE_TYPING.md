# Live typing: feasibility notes

**Status:** live translation while typing ships inside the DigitsNepal Translate screen. System-wide inline conversion does **not**, and DigitsNepal does not claim it works.

## Options considered

| Approach | How it works | Verdict |
|---|---|---|
| Global keystroke monitoring (`CGEventTap`, `NSEvent.addGlobalMonitor`) | Watch every key in every app, rewrite text | ❌ Rejected. It is effectively a keylogger, needs the Input Monitoring permission, sees passwords and private messages, and goes against the privacy design. |
| AX polling of the focused field | Periodically read `AXValue` and replace sentences | ❌ Rejected. It reads text the user didn't select, races with typing, breaks undo in the host app, and many apps don't support writes. |
| **InputMethodKit input method** (`IMKInputController`) | A Nepali→English input source the user switches to, like Pinyin. It receives keys only while selected, and composes in marked text that the user confirms. | ✅ The right long-term design. The user opts in per keystroke context (input source menu), it works in most Cocoa text views, and secure fields bypass it automatically. Cost: a separate `.app` bundle in `~/Library/Input Methods`, written in Swift or Objective-C, with its own signing, and LLM latency (0.5–5 s) inside an input method needs careful async candidate UI. |
| macOS Services menu ("Translate with DigitsNepal") | `NSServices` entry acting on the selection | ✅ Feasible follow-up. It complements the shortcut, but isn't "live". |
| **In-app live translation** | Type inside DigitsNepal; translate on pauses or sentence ends; copy out | ✅ **Implemented** (Translate screen, `src/lib/compose.ts`). Debounced (1.2 s pause, or immediately at `. ! ? ।`), never fires mid-word unless the pause is long, can be paused with a button, Esc, or the menu bar, and needs no extra permissions. |

## Next steps for an input method

1. A Swift IMK target `DigitsNepalIME.app` that talks to the main app over XPC, so it reuses the Rust engine via a small local service.
2. Show the Romanized text as marked text, with the English candidate in a candidate window after a pause. Return commits the English; Esc keeps the Nepali.
3. Verify by hand in TextEdit, Notes, Safari, Chrome, Slack, VS Code and Mail before advertising it.
