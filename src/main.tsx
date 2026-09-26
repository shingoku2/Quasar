import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ErrorBoundary from "./components/ErrorBoundary";
import { VaultProvider } from "./components/vault/VaultProvider";
import { getErrorMessage } from "./lib/utils";
import { applyStoredAppearance } from "./components/SettingsView";

const rootElement = document.getElementById("root");

if (!rootElement) {
  console.error("Root element not found!");
} else {
  // Saved theme/accent must apply at launch, not only once Settings → Appearance opens.
  applyStoredAppearance();
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
  } catch (error) {
    console.error("Failed to mount React app:", error);
    rootElement.replaceChildren();
    const wrapper = document.createElement('div');
    wrapper.style.color = 'red';
    wrapper.style.padding = '20px';

    const heading = document.createElement('h1');
    heading.textContent = 'Failed to load app';
    const message = document.createElement('pre');
    message.textContent = getErrorMessage(error);

    wrapper.append(heading, message);
    rootElement.appendChild(wrapper);
  }
}
