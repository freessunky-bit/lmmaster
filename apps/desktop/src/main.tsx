import React from "react";
import ReactDOM from "react-dom/client";
import "@lmmaster/design-system/tokens.css";
import "@lmmaster/design-system/base.css";
import "@lmmaster/design-system/components.css";
import "@lmmaster/design-system/react/pill.css";
import App from "./App";
import "./i18n/init";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
