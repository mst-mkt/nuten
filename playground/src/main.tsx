import "@fontsource-variable/onest";
import "./global.css";

import { Toast } from "@base-ui/react/toast";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app";
import { Toasts } from "./components/toasts/toasts";
import { toastManager } from "./lib/toasts";

const root = document.getElementById("root");
if (root === null) throw new Error("no element to render the playground into");

createRoot(root).render(
  <StrictMode>
    <Toast.Provider toastManager={toastManager} timeout={2000}>
      <App />
      <Toasts />
    </Toast.Provider>
  </StrictMode>,
);
