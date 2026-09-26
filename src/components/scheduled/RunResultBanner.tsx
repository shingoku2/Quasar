import React from 'react';

export interface RunResult {
  taskName: string;
  success: boolean;
  output: string | null;
  error: string | null;
}

/** The outcome of the last "Run now", until dismissed. */
const RunResultBanner: React.FC<{ result: RunResult; onDismiss: () => void }> = ({ result, onDismiss }) => (
  <div className="mb-4 p-4 rounded-xl bg-bg-card border border-border">
    <div className="flex items-start justify-between gap-2">
      <div className="min-w-0 flex-1">
        <div className="font-medium text-white">
          Run result: {result.taskName}
          {result.success ? (
            <span className="ml-2 text-success">Success</span>
          ) : (
            <span className="ml-2 text-alert">Failed</span>
          )}
        </div>
        {result.error && (
          <pre className="mt-2 text-sm text-alert whitespace-pre-wrap break-words font-mono">{result.error}</pre>
        )}
        {result.output != null && result.output.length > 0 && (
          <pre className="mt-2 text-sm text-gray-400 whitespace-pre-wrap break-words font-mono max-h-40 overflow-y-auto">{result.output}</pre>
        )}
      </div>
      <button
        type="button"
        onClick={onDismiss}
        className="shrink-0 text-gray-500 hover:text-white transition-colors"
        aria-label="Dismiss"
      >
        ×
      </button>
    </div>
  </div>
);

export default RunResultBanner;
