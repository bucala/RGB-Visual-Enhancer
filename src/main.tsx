import { StrictMode, lazy, Suspense } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./index.css";

// The tray menu is a second window loading the same bundle with ?view=tray.
const view = new URLSearchParams(window.location.search).get("view");
const TrayApp = lazy(() => import("./tray/TrayMenu"));
if (view === "tray") document.documentElement.classList.add("tray-view");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {view === "tray" ? (
      <Suspense fallback={null}>
        <TrayApp />
      </Suspense>
    ) : (
      <App />
    )}
  </StrictMode>,
);
