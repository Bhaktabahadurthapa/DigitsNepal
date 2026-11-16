import "@fontsource-variable/inter";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { Popover } from "./features/editor/Popover";
import "./styles/index.css";

const view = new URLSearchParams(window.location.search).get("view");
if (view === "popover") document.body.style.background = "transparent";

createRoot(document.getElementById("root")!).render(<StrictMode>{view === "popover" ? <Popover /> : <App />}</StrictMode>);
