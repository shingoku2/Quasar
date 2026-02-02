import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ErrorBoundary from "./components/ErrorBoundary";
import { VaultProvider } from "./components/vault/VaultProvider";

const rootElement = document.getElementById("root");

if (!rootElement) {
  console.error("Root element not found!");
} else {
  try {
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <ErrorBoundary>
          <VaultProvider>
            <App />
          </VaultProvider>
        </ErrorBoundary>
      </React.StrictMode>,
    );
    console.log("React app mounted successfully");
  } catch (error) {
    console.error("Failed to mount React app:", error);
    rootElement.innerHTML = `<div style="color: red; padding: 20px;">
      <h1>Failed to load app</h1>
      <pre>${error instanceof Error ? error.message : String(error)}</pre>
    </div>`;
  }
}
