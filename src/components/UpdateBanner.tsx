import React, { useState } from 'react';
import { Download, X } from 'lucide-react';
import { useUpdater } from '../hooks/useUpdater';

const UpdateBanner: React.FC = () => {
  const { status, version, error, installUpdate } = useUpdater(true);
  const [dismissed, setDismissed] = useState(false);

  if (dismissed || (status !== 'available' && status !== 'downloading' && status !== 'error')) {
    return null;
  }

  return (
    <div className="flex items-center justify-between px-4 py-2 bg-accent/10 border-b border-accent/30 text-sm shrink-0">
      <div className="flex items-center space-x-2">
        <Download className="h-4 w-4 text-accent shrink-0" />
        {status === 'error' ? (
          <span className="text-alert">Update check failed: {error}</span>
        ) : status === 'downloading' ? (
          <span className="text-white">Downloading update {version}…</span>
        ) : (
          <span className="text-white">Quasar {version} is available.</span>
        )}
      </div>
      <div className="flex items-center space-x-4">
        {status === 'available' && (
          <button
            type="button"
            onClick={installUpdate}
            className="text-accent font-medium hover:underline"
          >
            Install & Restart
          </button>
        )}
        <button
          type="button"
          onClick={() => setDismissed(true)}
          aria-label="Dismiss update notification"
          className="text-gray-400 hover:text-white"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
};

export default UpdateBanner;
