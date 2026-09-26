import React from 'react';
import type { SystemMetrics } from './types';

const DiskSpaceCard: React.FC<{ metrics: SystemMetrics }> = ({ metrics }) => (
  <div className="bg-bg-card border border-gray-800 rounded-xl p-6">
    <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest mb-4">Disk Space</h3>
    <div className="flex items-center justify-between mb-2">
      <span className="text-2xl font-black text-white">
        {metrics.disk_used_gb} GB / {metrics.disk_total_gb} GB
      </span>
      <span className="text-lg font-bold text-gray-400">
        {metrics.disk_usage_percent.toFixed(1)}%
      </span>
    </div>
    <div className="w-full bg-gray-800 rounded-full h-3 overflow-hidden">
      <div 
        className="h-full rounded-full transition-all duration-500"
        style={{ 
          width: `${metrics.disk_usage_percent}%`,
          backgroundColor: metrics.disk_usage_percent > 90 ? '#ef4444' : 
                          metrics.disk_usage_percent > 75 ? '#f59e0b' : '#10b981'
        }}
      />
    </div>
    <p className="text-xs text-gray-500 mt-2">
      {metrics.disk_free_gb} GB free
    </p>
  </div>
);

export default DiskSpaceCard;
