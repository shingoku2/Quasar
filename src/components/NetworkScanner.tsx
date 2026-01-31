import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { Play, Square, RefreshCw, Network, Search, Loader2 } from 'lucide-react';

export interface ScanResult {
  ip: string;
  is_alive: boolean;
  latency_ms?: number;
  open_ports: number[];
}

export interface ScanProgress {
  total: number;
  completed: number;
  current_ip?: string;
}

interface NetworkScannerProps {
  onResults?: (results: ScanResult[]) => void;
  onHostFound?: (host: ScanResult) => void;
  className?: string;
}

const DEFAULT_CIDR = "192.168.1.0/24";

const NetworkScanner: React.FC<NetworkScannerProps> = ({
  onResults,
  onHostFound,
  className = ''
}) => {
  const [cidr, setCidr] = useState(DEFAULT_CIDR);
  const [isScanning, setIsScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress>({ total: 0, completed: 0 });
  const [results, setResults] = useState<ScanResult[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Listen for scan events
  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenResult: UnlistenFn | null = null;

    const setupListeners = async () => {
      unlistenProgress = await listen<ScanProgress>('scan_progress', (event) => {
        setProgress(event.payload);
      });

      unlistenResult = await listen<ScanResult>('scan_result', (event) => {
        const result = event.payload;
        setResults(prev => [...prev, result]);
        
        if (result.is_alive && onHostFound) {
          onHostFound(result);
        }
      });
    };

    setupListeners();

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenResult) unlistenResult();
    };
  }, [onHostFound]);

  // Check scanning status periodically
  useEffect(() => {
    if (!isScanning) return;

    const interval = setInterval(async () => {
      try {
        const scanning = await invoke<boolean>('is_scanning');
        setIsScanning(scanning);
        
        if (!scanning) {
          // Scan completed
          if (onResults) {
            onResults(results);
          }
        }
      } catch (err) {
        console.error('Error checking scan status:', err);
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [isScanning, results, onResults]);

  const handleStartScan = async () => {
    try {
      setError(null);
      setResults([]);
      setProgress({ total: 0, completed: 0 });
      
      await invoke('scan_network', { cidr });
      setIsScanning(true);
    } catch (err) {
      setError(`Failed to start scan: ${err}`);
      console.error('Scan error:', err);
    }
  };

  const handleStopScan = async () => {
    try {
      await invoke('stop_scan');
      setIsScanning(false);
    } catch (err) {
      console.error('Stop scan error:', err);
    }
  };

  const handleClearResults = () => {
    setResults([]);
    setProgress({ total: 0, completed: 0 });
    setError(null);
  };

  const progressPercentage = progress.total > 0 
    ? Math.round((progress.completed / progress.total) * 100) 
    : 0;

  const aliveHosts = results.filter(r => r.is_alive);

  return (
    <div className={`bg-bg-root border border-gray-800 rounded-lg p-6 ${className}`}>
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center space-x-3">
          <Network className="h-5 w-5 text-accent" />
          <h2 className="text-lg font-bold text-white">Network Scanner</h2>
        </div>
        
        {isScanning && (
          <div className="flex items-center space-x-2 text-accent">
            <Loader2 className="h-4 w-4 animate-spin" />
            <span className="text-sm font-medium">Scanning...</span>
          </div>
        )}
      </div>

      {/* CIDR Input */}
      <div className="mb-6">
        <label className="block text-sm font-medium text-gray-400 mb-2">
          Target Network (CIDR)
        </label>
        <div className="flex space-x-2">
          <input
            type="text"
            value={cidr}
            onChange={(e) => setCidr(e.target.value)}
            placeholder="192.168.1.0/24"
            disabled={isScanning}
            className="flex-1 bg-bg-sidebar border border-gray-700 rounded-lg px-4 py-2 text-white text-sm focus:outline-none focus:border-accent disabled:opacity-50"
          />
          
          {!isScanning ? (
            <button
              onClick={handleStartScan}
              disabled={!cidr || isScanning}
              className="bg-accent hover:bg-accent/80 disabled:bg-gray-700 disabled:cursor-not-allowed text-white px-4 py-2 rounded-lg text-sm font-medium transition-all flex items-center space-x-2"
            >
              <Play className="h-4 w-4" />
              <span>Start</span>
            </button>
          ) : (
            <button
              onClick={handleStopScan}
              className="bg-red-600 hover:bg-red-700 text-white px-4 py-2 rounded-lg text-sm font-medium transition-all flex items-center space-x-2"
            >
              <Square className="h-4 w-4" />
              <span>Stop</span>
            </button>
          )}
        </div>
        
        {error && (
          <p className="mt-2 text-sm text-red-500">{error}</p>
        )}
      </div>

      {/* Progress */}
      {isScanning && progress.total > 0 && (
        <div className="mb-6">
          <div className="flex justify-between text-sm text-gray-400 mb-2">
            <span>Progress</span>
            <span>{progress.completed} / {progress.total} ({progressPercentage}%)</span>
          </div>
          <div className="w-full bg-gray-800 rounded-full h-2">
            <div 
              className="bg-accent h-2 rounded-full transition-all duration-300"
              style={{ width: `${progressPercentage}%` }}
            />
          </div>
          {progress.current_ip && (
            <p className="mt-1 text-xs text-gray-500">
              Scanning: {progress.current_ip}
            </p>
          )}
        </div>
      )}

      {/* Stats */}
      {results.length > 0 && (
        <div className="grid grid-cols-3 gap-4 mb-6">
          <div className="bg-bg-sidebar rounded-lg p-3 text-center">
            <p className="text-2xl font-bold text-white">{results.length}</p>
            <p className="text-xs text-gray-500 uppercase tracking-wider">Scanned</p>
          </div>
          <div className="bg-bg-sidebar rounded-lg p-3 text-center">
            <p className="text-2xl font-bold text-green-500">{aliveHosts.length}</p>
            <p className="text-xs text-gray-500 uppercase tracking-wider">Alive</p>
          </div>
          <div className="bg-bg-sidebar rounded-lg p-3 text-center">
            <p className="text-2xl font-bold text-blue-500">
              {aliveHosts.reduce((sum, h) => sum + (h.open_ports?.length || 0), 0)}
            </p>
            <p className="text-xs text-gray-500 uppercase tracking-wider">Open Ports</p>
          </div>
        </div>
      )}

      {/* Results List */}
      {aliveHosts.length > 0 && (
        <div className="border-t border-gray-800 pt-4">
          <div className="flex items-center justify-between mb-3">
            <h3 className="text-sm font-medium text-white flex items-center space-x-2">
              <Search className="h-4 w-4 text-gray-400" />
              <span>Discovered Hosts ({aliveHosts.length})</span>
            </h3>
            <button
              onClick={handleClearResults}
              className="text-xs text-gray-500 hover:text-white transition-colors flex items-center space-x-1"
            >
              <RefreshCw className="h-3 w-3" />
              <span>Clear</span>
            </button>
          </div>
          
          <div className="space-y-2 max-h-64 overflow-y-auto">
            {aliveHosts.map((host) => (
              <div 
                key={host.ip}
                className="bg-bg-sidebar rounded-lg p-3 flex items-center justify-between"
              >
                <div>
                  <p className="text-sm font-medium text-white">{host.ip}</p>
                  {host.latency_ms && (
                    <p className="text-xs text-gray-500">
                      Latency: {host.latency_ms}ms
                    </p>
                  )}
                </div>
                <div className="flex items-center space-x-2">
                  {host.open_ports && host.open_ports.length > 0 && (
                    <div className="flex space-x-1">
                      {host.open_ports.map(port => (
                        <span 
                          key={port}
                          className="text-[10px] bg-accent/20 text-accent px-2 py-0.5 rounded"
                        >
                          :{port}
                        </span>
                      ))}
                    </div>
                  )}
                  <div className="w-2 h-2 rounded-full bg-green-500" title="Alive" />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

export default NetworkScanner;
