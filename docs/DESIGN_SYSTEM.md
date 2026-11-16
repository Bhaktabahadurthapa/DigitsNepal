# DigitsNepal design system

Clean macOS light: a white canvas, a light grey sidebar, neutral greys and one
calm blue accent. Dark mode is a graphite version of the same system.

All tokens are generated from one brand color:

```bash
node scripts/generate-design-tokens.mjs "#0A6CFF" > src/styles/tokens.css
```

`tokens.css` is generated; change the script, not the file. `index.css` maps
the tokens to Tailwind utilities (`bg-surface`, `text-muted`, `bg-accent`…).

## Tokens

| Group | Tokens | Values |
|---|---|---|
| Brand scale | `--brand-50` … `--brand-900` | HSL ramp from `#0A6CFF`; 500 ≈ brand, 600+ passes 4.5:1 on white |
| Surfaces | `--bg`, `--surface`, `--surface-2`, `--sidebar` | `#FFFFFF`, `#FFFFFF`, `#F5F5F7`, `#F2F2F5` |
| Text | `--text`, `--muted` | `#1D1D1F` (16:1), `#6E6E73` (4.9:1) |
| Lines | `--border` | `#E3E3E8` |
| Accent | `--accent`, `--accent-hover`, `--accent-fg`, `--accent-soft`, `--on-accent` | fills, hover, blue text, tinted backgrounds, text on accent |
| Status | `--ok`/`--ok-bg`, `--warn-text`/`--warn-bg`, `--error-text`/`--error-bg` | success, attention, error |
| Type (×1.2 from 13 px) | `--text-xs` … `--text-2xl` | 11 · 12 · 13 · 15.5 · 18.5 · 22.5 · 27 px; Inter |
| Spacing (8-point) | `--space-0` … `--space-8` | 0 · 4 · 8 · 12 · 16 · 24 · 32 · 48 · 64 px |
| Radius | `--radius-sm/md/lg` | 6 · 8 · 12 px |
| Shadow | `--shadow-sm`, `--shadow-md` | resting cards, raised menus |
| Motion | `--duration-fast/base`, `--ease-standard` | 150 ms, 200 ms |

## Themes

`<html data-theme="light|dark|system">`. Light is the default; the choice
lives in Settings → Appearance and is remembered on the Mac. Components use
tokens only, never hard-coded colors, so both themes follow automatically.

## Components

| Component | Rules |
|---|---|
| Button | 32 px high, radius 8. `primary` = accent fill with `on-accent` text; `secondary` = surface + border; `ghost`; `danger` = error text. Disabled at 45 % opacity. |
| Sidebar item | 32 px, radius 6, icon 16 px. Active = subtle grey fill + accent icon. Grouped under small uppercase labels. |
| Card / Panel | Surface, 1 px border, radius 12, `shadow-sm`. Header 40 px. |
| Chat | User = accent bubble (radius 16, tail bottom-right). Assistant = plain text with a soft accent avatar. Tool steps = 20 px status dot + label. |
| Approval card | Accent border (warning border for always-ask actions), icon, action title, file path, reason, "Show details", then Allow once / Allow for this chat / Deny. |
| Badge | 24 px pill: green allowed, amber asked, red blocked. |
| Inputs | 32–36 px, radius 8, border turns accent on focus. |

## Accessibility

- Text contrast ≥ 4.5:1 in both themes; focus ring = 2 px accent outline.
- Icons are Lucide SVGs; icon-only buttons have `aria-label`. No emoji icons.
- Every input has a label; status changes use `aria-live`; reduced motion disables animations.
