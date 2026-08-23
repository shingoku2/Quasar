import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Activity, Cpu, HardDrive, Clock, X, AlertCircle, CheckCircle } from 'lucide-react';
import { getErrorMessage } from '../lib/utils';

interface HealthMetrics {
  cpu_percent?: number;
  memory_used_mb?: number;
  memory_total_mb?: number;
  disk_used_gb?: number;
  disk_total_gb?: number;
  uptime_seconds?: number;
  load_average?: number[];
}

interface HealthCheckResult {
  host: string;
  reachable: boolean;
  latency_ms?: number;
  metrics?: HealthMetrics;
  error?: string;
}

interface PreflightDialogProps {
  host: string;
  port: number;
  username: string;
  password?: string;
  onConnect: () => void;
  onCancel: () => void;
}

const PreflightDialog: React.FC<PreflightDialogProps> = ({
  host,
  port,
  username,
  password,
  onConnect,
  onCancel
}) => {
  const [result, setResult] = useState<HealthCheckResult | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const checkHealth = async () => {
      try {
        const healthResult = await invoke<HealthCheckResult>('check_host_health', {
          host,
          port,
          username,
          password
        });
        setResult(healthResult);
      } catch (err) {
        setResult({
          host,
          reachable: false,
          error: getErrorMessage(err)
        });
      } finally {
        setLoading(false);
      }
    };

    checkHealth();
  }, [host, port, username, password]);

  const formatUptime = (seconds?: number) => {
    if (!seconds) return 'Unknown';
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    if (days > 0) return `${days}d ${hours}h`;
    return `${hours}h`;
  };

  const formatBytes = (mb?: number, total?: number) => {
    if (!mb || !total) return 'Unknown';
    const usedGB = (mb / 1024).toFixed(1);
    const totalGB = (total / 1024).toFixed(1);
    return `${usedGB} / ${totalGB} GB`;
  };

  return (
    <div role="dialog"
      aria-modal="true"
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-bg-root border border-gray-700 rounded-lg w-full max-w-md mx-4">
        <div className="flex items-center justify-between p-4 border-b border-gray-700">
          <h2 className="text-lg font-bold text-white">Pre-flight Check</h2>
          <button onClick={onCancel} className="text-gray-400 hover:text-white">
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="p-4">
          {loading ? (
            <div className="flex items-center justify-center py-8">
              <Activity className="h-8 w-8 text-accent animate-pulse" />
            </div>
          ) : result?.reachable ? (
            <div className="space-y-4">
              <div className="flex items-center space-x-3 text-green-500">
                <CheckCircle className="h-6 w-6" />
                <div>
                  <p className="font-medium">Host is reachable</p>
                  {result.latency_ms && (
                    <p className="text-sm text-gray-400">Latency: {result.latency_ms}ms</p>
                  )}
                </div>
              </div>

              {result.metrics && (
                <div className="grid grid-cols-2 gap-3 mt-4">
                  {result.metrics.cpu_percent !== undefined && (
                    <div className="bg-bg-sidebar rounded-lg p-3">
                      <div className="flex items-center space-x-2 text-gray-400 mb-1">
                        <Cpu className="h-4 w-4" />
                        <span className="text-xs uppercase">CPU</span>
                      </div>
                      <p className="text-lg font-bold text-white">{result.metrics.cpu_percent.toFixed(1)}%</p>
                    </div>
                  )}

                  {result.metrics.memory_used_mb && (
                    <div className="bg-bg-sidebar rounded-lg p-3">
                      <div className="flex items-center space-x-2 text-gray-400 mb-1">
                        <Activity className="h-4 w-4" />
                        <span className="text-xs uppercase">Memory</span>
                      </div>
                      <p className="text-lg font-bold text-white">
                        {formatBytes(result.metrics.memory_used_mb, result.metrics.memory_total_mb)}
                      </p>
                    </div>
                  )}

                  {result.metrics.disk_used_gb && (
                    <div className="bg-bg-sidebar rounded-lg p-3">
                      <div className="flex items-center space-x-2 text-gray-400 mb-1">
                        <HardDrive className="h-4 w-4" />
                        <span className="text-xs uppercase">Disk</span>
                      </div>
                      <p className="text-lg font-bold text-white">{result.metrics.disk_used_gb} GB</p>
                    </div>
                  )}

                  {result.metrics.uptime_seconds && (
                    <div className="bg-bg-sidebar rounded-lg p-3">
                      <div className="flex items-center space-x-2 text-gray-400 mb-1">
                        <Clock className="h-4 w-4" />
                        <span className="text-xs uppercase">Uptime</span>
                      </div>
                      <p className="text-lg font-bold text-white">{formatUptime(result.metrics.uptime_seconds)}</p>
                    </div>
                  )}
                </div>
              )}
            </div>
          ) : (
            <div className="flex items-center space-x-3 text-red-500">
              <AlertCircle className="h-6 w-6" />
              <div>
                <p className="font-medium">Host unreachable</p>
                {result?.error && (
                  <p className="text-sm text-gray-400">{result.error}</p>
                )}
              </div>
            </div>
          )}
        </div>

        <div className="flex justify-end space-x-3 p-4 border-t border-gray-700">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-sm font-medium text-gray-400 hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={onConnect}
            disabled={!result?.reachable}
            className="px-4 py-2 bg-accent hover:bg-accent/80 disabled:bg-gray-700 disabled:cursor-not-allowed text-white text-sm font-medium rounded-lg transition-colors"
          >
            Connect
          </button>
        </div>
      </div>
    </div>
  );
};

export default PreflightDialog;
