import React from 'react';
import type { ProcessInfo } from './types';

/** A ranked process list: by CPU (percent) or by memory (MB). */
const TopProcessesCard: React.FC<{ kind: 'cpu' | 'memory'; processes: ProcessInfo[] }> = ({ kind, processes }) => (
  <div className="bg-bg-card border border-gray-800 rounded-xl p-6">
    <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest mb-4">
      {kind === 'cpu' ? 'Top CPU Processes' : 'Top Memory Processes'}
    </h3>
    <div className="space-y-3">
      {processes.map((proc, idx) => (
        <div key={`${kind}-${proc.pid}`} className="flex items-center justify-between">
          <div className="flex items-center space-x-3 flex-1 min-w-0">
            <span className="text-xs font-mono text-gray-500 w-6">#{idx + 1}</span>
            <span className="text-sm text-gray-300 truncate">{proc.name}</span>
          </div>
          <div className="flex items-center space-x-3">
            <span className="text-xs text-gray-500">PID {proc.pid}</span>
            {kind === 'cpu' ? (
              <span className="text-sm font-bold text-blue-400 w-16 text-right">{proc.cpu_usage.toFixed(1)}%</span>
            ) : (
              <span className="text-sm font-bold text-green-400 w-16 text-right">{proc.memory_mb} MB</span>
            )}
          </div>
        </div>
      ))}
    </div>
  </div>
);

export default TopProcessesCard;
