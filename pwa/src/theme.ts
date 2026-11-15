import type { Appearance } from "./types";

const COLORS = { light: "#f2ebdd", dark: "#16120f" };

export function applyAppearance(a: Appearance): void {
  const root = document.documentElement;
  if (a === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", a);
  for (const m of document.querySelectorAll<HTMLMetaElement>('meta[name="theme-color"]')) {
    const scheme = m.media.includes("dark") ? "dark" : "light";
    m.content = a === "system" ? COLORS[scheme] : COLORS[a];
  }
}
