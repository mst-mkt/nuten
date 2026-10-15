import "@fontsource-variable/onest";
import "./global.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app";

const root = document.getElementById("root");
if (root === null) throw new Error("no element to render the playground into");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
