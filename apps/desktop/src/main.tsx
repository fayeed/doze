import React from "react";
import { createRoot } from "react-dom/client";
import { brand } from "@doze/brand";
import "./style.css";

function App() {
  return (
    <main className="shell">
      <div className="moon" aria-hidden="true">☾</div>
      <p className="eyebrow">A quieter kind of utility</p>
      <h1>{brand.name}</h1>
      <p className="tagline">{brand.tagline}</p>
      <p className="note">A little more rest is on the way.</p>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
