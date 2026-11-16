import { useEffect, useState } from "react";

export type Theme = "light" | "dark" | "system";
const KEY = "dn.theme";

/** Light is the default; the choice is remembered on this Mac. */
export function readTheme(): Theme {
  try {
    const v = localStorage.getItem(KEY);
    return v === "dark" || v === "system" ? v : "light";
  } catch {
    return "light";
  }
}

export function applyTheme(t: Theme) {
  document.documentElement.dataset.theme = t;
}

export function useTheme(): [Theme, (t: Theme) => void] {
  const [theme, setTheme] = useState<Theme>(readTheme);
  useEffect(() => {
    applyTheme(theme);
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      /* storage unavailable: keep for this session */
    }
  }, [theme]);
  return [theme, setTheme];
}
