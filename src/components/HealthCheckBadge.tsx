import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Activity, AlertCircle, CheckCircle } from 'lucide-react';

interface HealthStatus {
  reachable: boolean;
  latency_ms?: number;
  error?: string;
}

interface HealthCheckBadgeProps {
  host: string;
  className?: string;
}

const HealthCheckBadge: React.FC<HealthCheckBadgeProps> = ({ host, className = '' }) => {
  const [status, setStatus] = useState<HealthStatus | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const checkHealth = async () => {
      setLoading(true);
      try {
        const result = await invoke<HealthStatus>('preflight_check', { host });
        setStatus(result);
      } catch (err) {
        setStatus({ reachable: false, error: 'Check failed' });
      } finally {
        setLoading(false);
      }
    };

    checkHealth();

    // Refresh every 30 seconds
    const interval = setInterval(checkHealth, 30000);
    return () => clearInterval(interval);
  }, [host]);

  if (loading && !status) {
    return (
      <span className={`inline-flex items-center text-gray-500 ${className}`}>
        <Activity className="h-3 w-3 animate-pulse" />
      </span>
    );
  }

  if (!status) {
    return null;
  }

  if (status.reachable) {
    return (
      <span 
        className={`inline-flex items-center space-x-1 text-green-500 ${className}`}
        title={`Reachable${status.latency_ms ? ` (${status.latency_ms}ms)` : ''}`}
      >
        <CheckCircle className="h-3 w-3" />
        {status.latency_ms && (
          <span className="text-[10px]">{status.latency_ms}ms</span>
        )}
      </span>
    );
  }

  return (
    <span 
      className={`inline-flex items-center text-red-500 ${className}`}
      title={status.error || 'Unreachable'}
    >
      <AlertCircle className="h-3 w-3" />
    </span>
  );
};

export default HealthCheckBadge;
