import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { History, Play, CheckCircle, XCircle, Clock, AlertCircle, Terminal, RotateCw } from 'lucide-react';

export interface ExecutionRecord {
  id: string;
  workflow_id: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  started_at: string;
  completed_at?: string;
  error_message?: string;
  node_results?: Record<string, NodeExecutionResult>;
}

export interface NodeExecutionResult {
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  output?: string;
  error?: string;
  started_at: string;
  completed_at?: string;
}

interface ExecutionHistoryProps {
  workflowId?: string;
}

const ExecutionHistory: React.FC<ExecutionHistoryProps> = ({ workflowId }) => {
  const [executions, setExecutions] = useState<ExecutionRecord[]>([]);
  const [selectedExecution, setSelectedExecution] = useState<ExecutionRecord | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (workflowId) {
      loadExecutions();
    }

    // Listen for execution events
    const unlistenStarted = listen('workflow-execution-started', (event) => {
      console.log('Execution started:', event.payload);
      loadExecutions();
    });

    const unlistenCompleted = listen('workflow-execution-completed', (event) => {
      console.log('Execution completed:', event.payload);
      loadExecutions();
    });

    const unlistenNode = listen('workflow-node-completed', (event) => {
      console.log('Node completed:', event.payload);
    });

    return () => {
      unlistenStarted.then(fn => fn());
      unlistenCompleted.then(fn => fn());
      unlistenNode.then(fn => fn());
    };
  }, [workflowId]);

  const loadExecutions = async () => {
    setLoading(true);
    try {
      const result = await invoke<ExecutionRecord[]>('list_executions', {
        workflowId: workflowId || null,
      });
      // Sort by started_at descending (most recent first)
      const sorted = result.sort((a, b) => 
        new Date(b.started_at).getTime() - new Date(a.started_at).getTime()
      );
      setExecutions(sorted);
    } catch (err) {
      console.error('Failed to load executions:', err);
    } finally {
      setLoading(false);
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'completed':
        return <CheckCircle className="w-4 h-4 text-green-500" />;
      case 'failed':
        return <XCircle className="w-4 h-4 text-red-500" />;
      case 'running':
        return <RotateCw className="w-4 h-4 text-blue-500 animate-spin" />;
      case 'cancelled':
        return <AlertCircle className="w-4 h-4 text-yellow-500" />;
      default:
        return <Clock className="w-4 h-4 text-gray-500" />;
    }
  };

  const getStatusClass = (status: string) => {
    switch (status) {
      case 'completed':
        return 'bg-green-500/10 text-green-500 border-green-500/20';
      case 'failed':
        return 'bg-red-500/10 text-red-500 border-red-500/20';
      case 'running':
        return 'bg-blue-500/10 text-blue-500 border-blue-500/20';
      case 'cancelled':
        return 'bg-yellow-500/10 text-yellow-500 border-yellow-500/20';
      default:
        return 'bg-gray-500/10 text-gray-500 border-gray-500/20';
    }
  };

  const formatDuration = (start: string, end?: string) => {
    const startDate = new Date(start);
    const endDate = end ? new Date(end) : new Date();
    const diff = endDate.getTime() - startDate.getTime();
    const seconds = Math.floor(diff / 1000);
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    return `${minutes}m ${seconds % 60}s`;
  };

  const formatTime = (timestamp: string) => {
    return new Date(timestamp).toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  };

  return (
    <div className="flex h-full bg-zinc-950">
      {/* Execution List */}
      <div className="w-80 border-r border-gray-800 flex flex-col">
        {/* Header */}
        <div className="h-14 border-b border-gray-800 flex items-center justify-between px-4">
          <div className="flex items-center space-x-2">
            <History className="w-4 h-4 text-accent" />
            <h3 className="font-bold text-sm">Execution History</h3>
          </div>
          <button
            onClick={loadExecutions}
            className="p-1.5 hover:bg-white/10 rounded text-gray-400 hover:text-white"
            disabled={loading}
          >
            <RotateCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
          </button>
        </div>

        {/* List */}
        <div className="flex-1 overflow-y-auto p-2 space-y-2">
          {executions.length === 0 ? (
            <div className="text-center py-8 text-gray-500">
              <History className="w-12 h-12 mx-auto mb-3 opacity-30" />
              <p className="text-sm">No executions yet</p>
              <p className="text-xs mt-1">Run a workflow to see history</p>
            </div>
          ) : (
            executions.map((execution) => (
              <button
                key={execution.id}
                onClick={() => setSelectedExecution(execution)}
                className={`w-full p-3 rounded-lg border text-left transition-all ${
                  selectedExecution?.id === execution.id
                    ? 'border-accent bg-accent/10'
                    : 'border-gray-800 hover:border-gray-700 hover:bg-white/5'
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-2">
                    {getStatusIcon(execution.status)}
                    <span className="text-sm font-medium">
                      {formatTime(execution.started_at)}
                    </span>
                  </div>
                  <span
                    className={`text-[10px] uppercase px-2 py-0.5 rounded border ${getStatusClass(
                      execution.status
                    )}`}
                  >
                    {execution.status}
                  </span>
                </div>
                <div className="mt-2 text-xs text-gray-500 flex items-center space-x-3">
                  <span>Duration: {formatDuration(execution.started_at, execution.completed_at)}</span>
                  {execution.node_results && (
                    <span>{Object.keys(execution.node_results).length} nodes</span>
                  )}
                </div>
              </button>
            ))
          )}
        </div>
      </div>

      {/* Execution Details */}
      <div className="flex-1 flex flex-col">
        {selectedExecution ? (
          <>
            {/* Detail Header */}
            <div className="h-14 border-b border-gray-800 flex items-center justify-between px-4">
              <div className="flex items-center space-x-3">
                {getStatusIcon(selectedExecution.status)}
                <div>
                  <h3 className="font-bold text-sm">Execution Details</h3>
                  <p className="text-xs text-gray-500">ID: {selectedExecution.id}</p>
                </div>
              </div>
              <div className="text-xs text-gray-500">
                Started: {formatTime(selectedExecution.started_at)}
              </div>
            </div>

            {/* Detail Content */}
            <div className="flex-1 overflow-y-auto p-4">
              {/* Summary */}
              <div className="grid grid-cols-4 gap-4 mb-6">
                <div className="bg-bg-card border border-gray-800 rounded-lg p-3">
                  <div className="text-xs text-gray-500 uppercase">Status</div>
                  <div className={`text-sm font-medium mt-1 ${getStatusClass(selectedExecution.status).split(' ')[1]}`}>
                    {selectedExecution.status}
                  </div>
                </div>
                <div className="bg-bg-card border border-gray-800 rounded-lg p-3">
                  <div className="text-xs text-gray-500 uppercase">Duration</div>
                  <div className="text-sm font-medium mt-1">
                    {formatDuration(selectedExecution.started_at, selectedExecution.completed_at)}
                  </div>
                </div>
                <div className="bg-bg-card border border-gray-800 rounded-lg p-3">
                  <div className="text-xs text-gray-500 uppercase">Nodes</div>
                  <div className="text-sm font-medium mt-1">
                    {selectedExecution.node_results
                      ? Object.keys(selectedExecution.node_results).length
                      : 0}
                  </div>
                </div>
                <div className="bg-bg-card border border-gray-800 rounded-lg p-3">
                  <div className="text-xs text-gray-500 uppercase">Completed</div>
                  <div className="text-sm font-medium mt-1 text-green-500">
                    {selectedExecution.node_results
                      ? Object.values(selectedExecution.node_results).filter(
                          (r) => r.status === 'completed'
                        ).length
                      : 0}
                  </div>
                </div>
              </div>

              {/* Error Message */}
              {selectedExecution.error_message && (
                <div className="bg-red-500/10 border border-red-500/20 rounded-lg p-3 mb-4">
                  <div className="flex items-center space-x-2 text-red-400 mb-2">
                    <AlertCircle className="w-4 h-4" />
                    <span className="text-sm font-medium">Error</span>
                  </div>
                  <p className="text-sm text-red-300 font-mono">{selectedExecution.error_message}</p>
                </div>
              )}

              {/* Node Results */}
              {selectedExecution.node_results && (
                <div>
                  <h4 className="text-xs font-medium text-gray-400 uppercase mb-3">Node Results</h4>
                  <div className="space-y-2">
                    {Object.entries(selectedExecution.node_results).map(([nodeId, result]) => (
                      <div
                        key={nodeId}
                        className="bg-bg-card border border-gray-800 rounded-lg p-3"
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex items-center space-x-2">
                            <Terminal className="w-4 h-4 text-gray-500" />
                            <span className="text-sm font-medium">{nodeId}</span>
                          </div>
                          <div className="flex items-center space-x-3">
                            {getStatusIcon(result.status)}
                            <span className="text-xs text-gray-500">
                              {formatDuration(result.started_at, result.completed_at)}
                            </span>
                          </div>
                        </div>
                        {result.output && (
                          <div className="mt-2 p-2 bg-zinc-900 rounded text-xs font-mono text-gray-400">
                            {result.output}
                          </div>
                        )}
                        {result.error && (
                          <div className="mt-2 p-2 bg-red-500/10 rounded text-xs font-mono text-red-400">
                            {result.error}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center text-gray-500">
              <Play className="w-16 h-16 mx-auto mb-4 opacity-30" />
              <p className="text-lg font-medium">Select an execution</p>
              <p className="text-sm mt-2">Choose an execution from the list to view details</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default ExecutionHistory;
