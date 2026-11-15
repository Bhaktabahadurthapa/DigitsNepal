export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Older Safari fallback.
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    ta.setSelectionRange(0, text.length);
    let ok = false;
    try { ok = document.execCommand("copy"); } catch { ok = false; }
    ta.remove();
    return ok;
  }
}

export const canShare = () => typeof navigator !== "undefined" && typeof navigator.share === "function";

/** Returns "shared", "cancelled", or "copied" (fallback when sharing is unavailable). */
export async function shareText(text: string): Promise<"shared" | "cancelled" | "copied" | "failed"> {
  if (canShare()) {
    try {
      await navigator.share({ text });
      return "shared";
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") return "cancelled";
    }
  }
  return (await copyText(text)) ? "copied" : "failed";
}
