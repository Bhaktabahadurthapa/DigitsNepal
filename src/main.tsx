import "@fontsource-variable/inter";
import { applyTheme, readTheme } from "./hooks/useTheme";

applyTheme(readTheme());
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { Popover } from "./features/editor/Popover";
import { TypingButton } from "./features/editor/TypingButton";
import "./styles/index.css";

const view = new URLSearchParams(window.location.search).get("view");
if (view === "popover" || view === "bubble") {
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>{view === "popover" ? <Popover /> : view === "bubble" ? <TypingButton /> : <App />}</StrictMode>,
);
