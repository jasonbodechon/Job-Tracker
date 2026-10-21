import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import Root from "./Root";
import "./styles.css";
import "./features.css";

if ("serviceWorker" in navigator) window.addEventListener("load", () => navigator.serviceWorker.register("/sw.js").catch(console.error));
createRoot(document.getElementById("root")!).render(<StrictMode><Root /></StrictMode>);
