import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Popup } from "./popup/Popup";
import { Dashboard } from "./dashboard/Dashboard";
import "./styles/app.css";

function pickEntry(): "popup" | "dashboard" {
  try {
    const label = getCurrentWindow().label;
    if (label === "dashboard") return "dashboard";
    if (label === "tray-popup") return "popup";
  } catch {
    /* fall through */
  }
  if (typeof window !== "undefined" && window.location.hash.startsWith("#/dashboard")) {
    return "dashboard";
  }
  return "popup";
}

const entry = pickEntry();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {entry === "popup" ? <Popup /> : <Dashboard />}
  </React.StrictMode>,
);
