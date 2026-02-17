import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { Play, Square, RefreshCw, Network, Search, Loader2, Server, Laptop, Router, Printer, HelpCircle } from 'lucide-react';

export interface ServiceInfo {
  port: number;
  protocol: string;
  service: string;
  version?: string;
}

export interface ScanResult {
  ip: string;
  is_alive: boolean;
  latency_ms?: number;
  open_ports: number[];
  hostname?: string;
  mac_address?: string;
  device_type: string;
  services: ServiceInfo[];
  vendor?: string;
  last_seen: number;
}

export interface ScanProgress {
  total: number;
  completed: number;
  current_ip?: string;
}

interface NetworkScannerProps {
  /** Persisted / last-scan results to show when not scanning (e.g. from get_discovered_hosts on load). */
  initialResults?: ScanResult[];
  onResults?: (results: ScanResult[]) => void;
  onHostFound?: (host: ScanResult) => void;
  onHostClick?: (host: ScanResult) => void;
  className?: string;
}

const DEFAULT_CIDR = "192.168.1.0/24";

const NetworkScanner: React.FC<NetworkScannerProps> = ({
  initialResults,
  onResults,
  onHostFound,
  onHostClick,
  className = ''
}) => {
  const [cidr, setCidr] = useState(DEFAULT_CIDR);
  const [isScanning, setIsScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress>({ total: 0, completed: 0 });
  const [results, setResults] = useState<ScanResult[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [scanCompleted, setScanCompleted] = useState(false);
  const resultsRef = useRef<ScanResult[]>([]);

  // Listen for scan events
  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenResult: UnlistenFn | null = null;
    let unlistenComplete: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;

    const setupListeners = async () => {
      try {
        unlistenProgress = await listen<ScanProgress>('scan_progress', (event) => {
          setProgress(event.payload);
        });

        unlistenResult = await listen<ScanResult>('scan_result', (event) => {
          const result = event.payload;
          setResults(prev => {
            // Deduplicate by IP address - keep the latest result
            const filtered = prev.filter(r => r.ip !== result.ip);
            const next = [...filtered, result];
            resultsRef.current = next;
            return next;
          });
          
          if (result.is_alive && onHostFound) {
            onHostFound(result);
          }
        });

        unlistenComplete = await listen('scan_complete', () => {
          setScanCompleted(true);
          setIsScanning(false);
        });

        unlistenError = await listen<string>('scan_error', (event) => {
          setError(event.payload);
        });
      } catch (error) {
        console.warn('Scan event listeners failed:', error);
      }
    };

    setupListeners();

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenResult) unlistenResult();
      if (unlistenComplete) unlistenComplete();
      if (unlistenError) unlistenError();
    };
  }, [onHostFound]);

  // When not scanning, keep list in sync with parent (e.g. persisted load, or after a host is deleted). Sync when lengths differ so deletions apply; avoid overwriting when lengths match (e.g. scan just finished, parent not yet re-rendered).
  useEffect(() => {
    if (isScanning || !initialResults) return;
    if (results.length === 0 || initialResults.length !== results.length) {
      setResults([...initialResults]);
      resultsRef.current = [...initialResults];
    }
  }, [initialResults, isScanning]);

  // Finalize results on completion
  useEffect(() => {
    if (!scanCompleted) return;
    if (onResults) {
      onResults(resultsRef.current);
    }
    setScanCompleted(false);
  }, [scanCompleted, onResults]);

  const handleStartScan = async () => {
    try {
      setError(null);
      setResults([]);
      resultsRef.current = [];
      setProgress({ total: 0, completed: 0 });
      setScanCompleted(false);
      
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

  const getDeviceIcon = (deviceType: string) => {
    switch (deviceType) {
      case 'server':
        return <Server className="h-4 w-4" />;
      case 'router':
        return <Router className="h-4 w-4" />;
      case 'printer':
        return <Printer className="h-4 w-4" />;
      case 'workstation':
        return <Laptop className="h-4 w-4" />;
      default:
        return <HelpCircle className="h-4 w-4" />;
    }
  };

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
                onClick={() => onHostClick?.(host)}
                className="bg-bg-sidebar rounded-lg p-3 flex items-center justify-between hover:bg-bg-sidebar/80 cursor-pointer transition-colors"
              >
                <div className="flex items-center space-x-3 flex-1">
                  <div className="text-gray-400">
                    {getDeviceIcon(host.device_type)}
                  </div>
                  <div className="flex-1">
                    <div className="flex items-center space-x-2">
                      <p className="text-sm font-medium text-white">{host.ip}</p>
                      {host.hostname && (
                        <span className="text-xs text-gray-500">({host.hostname})</span>
                      )}
                    </div>
                    <div className="flex items-center space-x-2 mt-1">
                      <span className="text-xs text-gray-500 capitalize">{host.device_type}</span>
                      {host.latency_ms && (
                        <>
                          <span className="text-gray-700">•</span>
                          <span className="text-xs text-gray-500">{host.latency_ms}ms</span>
                        </>
                      )}
                      {host.services.length > 0 && (
                        <>
                          <span className="text-gray-700">•</span>
                          <span className="text-xs text-gray-500">{host.services.length} services</span>
                        </>
                      )}
                    </div>
                  </div>
                </div>
                <div className="flex items-center space-x-2">
                  {host.open_ports && host.open_ports.length > 0 && (
                    <div className="flex space-x-1">
                      {host.open_ports.slice(0, 3).map(port => (
                        <span 
                          key={port}
                          className="text-[10px] bg-accent/20 text-accent px-2 py-0.5 rounded"
                        >
                          :{port}
                        </span>
                      ))}
                      {host.open_ports.length > 3 && (
                        <span className="text-[10px] text-gray-500 px-2 py-0.5">
                          +{host.open_ports.length - 3}
                        </span>
                      )}
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
