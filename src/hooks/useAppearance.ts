import { useEffect } from "react";
import type { Settings } from "../types/translation";

/** Apply window opacity from settings (background only; text stays solid). */
export function useAppearance(settings: Settings | null) {
  const opacity = settings?.window_opacity ?? 100;
  useEffect(() => {
    const alpha = Math.min(100, Math.max(30, opacity)) / 100;
    document.documentElement.style.setProperty("--alpha", String(alpha));
    document.documentElement.classList.toggle("glass", alpha < 1);
  }, [opacity]);
}
