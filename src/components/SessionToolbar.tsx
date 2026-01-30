import React from 'react';
import { Clipboard, Activity, Zap, ShieldCheck, ShieldAlert } from 'lucide-react';

interface SessionToolbarProps {
  sessionId: string;
  latency?: number;
  bandwidth?: string;
  clipboardSync: boolean;
  onToggleClipboard: () => void;
  className?: string;
}

const SessionToolbar: React.FC<SessionToolbarProps> = ({
  latency,
  bandwidth,
  clipboardSync,
  onToggleClipboard,
  className
}) => {
  return (
    <div className={`flex items-center justify-between px-3 py-1.5 bg-bg-sidebar border-b border-gray-800 text-[10px] font-mono uppercase tracking-wider ${className || ''}`}>
      <div className="flex items-center space-x-4">
        {/* Latency / Health */}
        <div className="flex items-center space-x-1.5 text-gray-400">
          <Activity className={`h-3 w-3 ${latency && latency < 100 ? 'text-green-500' : 'text-yellow-500'}`} />
          <span>Latency: <span className="text-gray-200">{latency !== undefined ? `${latency}ms` : '---'}</span></span>
        </div>

        {/* Bandwidth */}
        <div className="flex items-center space-x-1.5 text-gray-400">
          <Zap className="h-3 w-3 text-blue-500" />
          <span>Traffic: <span className="text-gray-200">{bandwidth || '---'}</span></span>
        </div>
      </div>

      <div className="flex items-center space-x-3">
        {/* Clipboard Sync Toggle */}
        <button
          onClick={onToggleClipboard}
          className={`flex items-center space-x-1.5 px-2 py-0.5 rounded border transition-all ${
            clipboardSync 
              ? 'bg-accent/10 border-accent text-accent' 
              : 'bg-transparent border-gray-700 text-gray-500 hover:border-gray-500'
          }`}
          aria-label="Toggle Clipboard Sync"
        >
          <Clipboard className="h-3 w-3" />
          <span>Clipboard {clipboardSync ? <ShieldCheck className="inline h-2 w-2 ml-0.5" /> : <ShieldAlert className="inline h-2 w-2 ml-0.5" />}</span>
        </button>
      </div>
    </div>
  );
};

export default SessionToolbar;
