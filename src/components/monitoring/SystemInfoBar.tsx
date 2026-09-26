import React from 'react';
import { formatUptime } from './format';
import type { SystemMetrics } from './types';

const SystemInfoBar: React.FC<{ metrics: SystemMetrics }> = ({ metrics }) => (
  <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
    <div className="bg-bg-card border border-gray-800 rounded-lg p-4">
      <p className="text-xs text-gray-500 uppercase tracking-wider">Uptime</p>
      <p className="text-xl font-bold text-white mt-1">{formatUptime(metrics.uptime_seconds)}</p>
    </div>
    <div className="bg-bg-card border border-gray-800 rounded-lg p-4">
      <p className="text-xs text-gray-500 uppercase tracking-wider">Load Avg</p>
      <p className="text-xl font-bold text-white mt-1">
        {metrics.load_average_1m.toFixed(2)}
      </p>
      <p className="text-xs text-gray-500 mt-1">
        {metrics.load_average_5m.toFixed(2)} / {metrics.load_average_15m.toFixed(2)}
      </p>
    </div>
    <div className="bg-bg-card border border-gray-800 rounded-lg p-4">
      <p className="text-xs text-gray-500 uppercase tracking-wider">Processes</p>
      <p className="text-xl font-bold text-white mt-1">{metrics.process_count}</p>
    </div>
    <div className="bg-bg-card border border-gray-800 rounded-lg p-4">
      <p className="text-xs text-gray-500 uppercase tracking-wider">CPU Cores</p>
      <p className="text-xl font-bold text-white mt-1">{metrics.cpu_count}</p>
      <p className="text-xs text-gray-500 mt-1">{metrics.cpu_frequency_mhz} MHz</p>
    </div>
  </div>
);

export default SystemInfoBar;
