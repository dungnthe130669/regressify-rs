import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";
import App from "./App";
import { enhanceLandingPage } from "./landing";

const root = document.getElementById("root");
if (!root) throw new Error("Missing application root.");
if (!enhanceLandingPage())
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
