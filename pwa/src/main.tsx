import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./fonts.css";
import "./styles.css";
import { applyAppearance } from "./theme";
import { loadPrefs } from "./storage";

applyAppearance(loadPrefs().appearance);
createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);

if (import.meta.env.PROD && "serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    navigator.serviceWorker.register("/app/sw.js", { scope: "/app/" }).catch(() => { /* app still works online */ });
  });
}
