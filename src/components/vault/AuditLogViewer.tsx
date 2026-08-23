import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FileText, Search, Filter, AlertCircle, CheckCircle, XCircle, Info } from 'lucide-react';
import { getErrorMessage } from '../../lib/utils';

interface AuditLogEntry {
  id: string;
  timestamp: number;
  event_type: string;
  resource_id?: string;
  resource_type?: string;
  action: string;
  result: string;
  details?: string;
  user_context?: string;
}

const AuditLogViewer: React.FC = () => {
  const [logs, setLogs] = useState<AuditLogEntry[]>([]);
  const [filteredLogs, setFilteredLogs] = useState<AuditLogEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [filterType, setFilterType] = useState<string>('all');
  const [filterResult, setFilterResult] = useState<string>('all');
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState('');

  useEffect(() => {
    loadAuditLogs();
  }, []);

  useEffect(() => {
    applyFilters();
  }, [logs, searchQuery, filterType, filterResult]);

  const loadAuditLogs = async () => {
    setIsLoading(true);
    setError('');
    try {
      const auditLogs = await invoke<AuditLogEntry[]>('get_audit_logs', {
        filter: {
          limit: 100,
          offset: 0
        }
      });
      setLogs(auditLogs);
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to load audit logs'));
    } finally {
      setIsLoading(false);
    }
  };

  const applyFilters = () => {
    let filtered = [...logs];

    if (searchQuery) {
      filtered = filtered.filter(
        (log) =>
          log.action.toLowerCase().includes(searchQuery.toLowerCase()) ||
          log.event_type.toLowerCase().includes(searchQuery.toLowerCase()) ||
          (log.details && log.details.toLowerCase().includes(searchQuery.toLowerCase()))
      );
    }

    if (filterType !== 'all') {
      filtered = filtered.filter((log) => log.event_type === filterType);
    }

    if (filterResult !== 'all') {
      filtered = filtered.filter((log) => log.result === filterResult);
    }

    setFilteredLogs(filtered);
  };

  const getEventIcon = (eventType: string) => {
    switch (eventType.toLowerCase()) {
      case 'credential_access':
        return <Info className="h-4 w-4 text-blue-400" />;
      case 'vault_unlock':
      case 'vault_lock':
        return <AlertCircle className="h-4 w-4 text-warning" />;
      case 'credential_create':
      case 'credential_update':
        return <CheckCircle className="h-4 w-4 text-success" />;
      case 'credential_delete':
        return <XCircle className="h-4 w-4 text-alert" />;
      default:
        return <FileText className="h-4 w-4 text-gray-400" />;
    }
  };

  const getResultColor = (result: string) => {
    switch (result.toLowerCase()) {
      case 'success':
        return 'text-success';
      case 'failure':
        return 'text-alert';
      case 'denied':
        return 'text-warning';
      default:
        return 'text-gray-400';
    }
  };

  const formatTimestamp = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const eventTypes = ['all', 'vault_unlock', 'vault_lock', 'credential_access', 'credential_create', 'credential_update', 'credential_delete'];
  const resultTypes = ['all', 'success', 'failure', 'denied'];

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-gray-800">
        <div className="flex justify-between items-center mb-4">
          <div className="flex items-center space-x-2">
            <FileText className="h-5 w-5 text-accent" />
            <h1 className="text-xl font-bold text-white">Security Audit Log</h1>
          </div>
          <div className="text-sm text-gray-500">
            {filteredLogs.length} {filteredLogs.length === 1 ? 'entry' : 'entries'}
          </div>
        </div>

        <div className="space-y-3">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500" />
            <input
              type="text"
              placeholder="Search audit logs..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full bg-bg-sidebar border border-gray-700 rounded-lg pl-10 pr-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all placeholder:text-gray-600"
            />
          </div>

          <div className="flex space-x-2">
            <div className="flex-1">
              <div className="flex items-center space-x-2 mb-1">
                <Filter className="h-3 w-3 text-gray-500" />
                <label className="text-xs text-gray-500">Event Type</label>
              </div>
              <select
                value={filterType}
                onChange={(e) => setFilterType(e.target.value)}
                className="w-full bg-bg-sidebar border border-gray-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
              >
                {eventTypes.map((type) => (
                  <option key={type} value={type}>
                    {type.replace('_', ' ').replace(/\b\w/g, (l) => l.toUpperCase())}
                  </option>
                ))}
              </select>
            </div>

            <div className="flex-1">
              <div className="flex items-center space-x-2 mb-1">
                <Filter className="h-3 w-3 text-gray-500" />
                <label className="text-xs text-gray-500">Result</label>
              </div>
              <select
                value={filterResult}
                onChange={(e) => setFilterResult(e.target.value)}
                className="w-full bg-bg-sidebar border border-gray-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
              >
                {resultTypes.map((type) => (
                  <option key={type} value={type}>
                    {type.charAt(0).toUpperCase() + type.slice(1)}
                  </option>
                ))}
              </select>
            </div>
          </div>
        </div>
      </div>

      {error && (
        <div className="mx-6 mt-4 bg-alert/10 border border-alert/30 rounded-lg px-4 py-3 text-alert text-sm">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-auto p-6">
        {isLoading ? (
          <div className="text-center text-gray-500 py-12">Loading audit logs...</div>
        ) : filteredLogs.length === 0 ? (
          <div className="text-center text-gray-500 py-12">
            <FileText className="h-12 w-12 mx-auto mb-4 opacity-50" />
            <p>{searchQuery || filterType !== 'all' || filterResult !== 'all' ? 'No matching audit logs found' : 'No audit logs yet'}</p>
            <p className="text-sm mt-2">Security events will be logged here</p>
          </div>
        ) : (
          <div className="space-y-2">
            {filteredLogs.map((log) => (
              <div
                key={log.id}
                className="bg-bg-sidebar border border-gray-700 rounded-lg p-4 hover:border-accent/50 transition-all"
              >
                <div className="flex items-start space-x-3">
                  <div className="shrink-0 mt-1">{getEventIcon(log.event_type)}</div>
                  
                  <div className="flex-1 min-w-0">
                    <div className="flex items-start justify-between mb-2">
                      <div className="flex-1">
                        <h3 className="text-white font-medium text-sm">
                          {log.action}
                        </h3>
                        <p className="text-xs text-gray-500 mt-0.5">
                          {formatTimestamp(log.timestamp)}
                        </p>
                      </div>
                      <span className={`text-xs font-medium uppercase px-2 py-1 rounded ${getResultColor(log.result)}`}>
                        {log.result}
                      </span>
                    </div>

                    <div className="space-y-1 text-xs">
                      <div className="flex items-center space-x-2 text-gray-400">
                        <span className="font-medium">Type:</span>
                        <span className="font-mono">{log.event_type}</span>
                      </div>
                      
                      {log.resource_type && (
                        <div className="flex items-center space-x-2 text-gray-400">
                          <span className="font-medium">Resource:</span>
                          <span className="font-mono">
                            {log.resource_type}
                            {log.resource_id && ` (${log.resource_id.substring(0, 8)}...)`}
                          </span>
                        </div>
                      )}

                      {log.details && (
                        <div className="mt-2 bg-bg-root border border-gray-700 rounded px-3 py-2">
                          <p className="text-gray-300">{log.details}</p>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

export default AuditLogViewer;
