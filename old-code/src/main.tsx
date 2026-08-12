import React from "react";
import ReactDOM from "react-dom/client";
import { isDevelopmentMode } from "./core/utils/env";
import App from "./App";
import "./App.css";

if (isDevelopmentMode()) {
  console.log("Running in development mode");
}

const appElement = <App />;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  isDevelopmentMode() ? (
    <React.StrictMode>{appElement}</React.StrictMode>
  ) : (
    appElement
  )
);
