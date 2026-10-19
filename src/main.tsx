import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { MiniHud } from "./components/MiniHud";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("MemFlux root element is missing.");

// The Mini HUD webview loads the same bundle with `?hud=mini` and gets a
// compact standalone surface instead of the full dashboard.
const isMiniHud = new URLSearchParams(window.location.search).get("hud") === "mini";

createRoot(root).render(
  <StrictMode>
    {isMiniHud ? <MiniHud /> : <App />}
  </StrictMode>
);

