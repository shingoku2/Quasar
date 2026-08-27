import React, { useState, useEffect } from 'react';
import { Bell, Plus, Trash2, Check } from 'lucide-react';
import { cn } from '../../lib/utils';
import { invoke } from '@tauri-apps/api/core';

/**
 * Alert rule as it crosses Tauri IPC. `metric`/`operator`/`severity` are fieldless
 * Rust enums (monitoring.rs `MetricType`/`ComparisonOperator`/`AlertSeverity`) with
 * no custom serde tagging, so serde serializes them as bare strings (e.g. "CpuUsage"),
 * not as `{ CpuUsage: null }` — that shape is only something serde's deserializer
 * happens to also accept, not what it actually sends.
 */
export interface AlertRule {
  id: string;
  metric: 'CpuUsage' | 'MemoryUsage' | 'DiskUsage';
  operator: 'GreaterThan' | 'LessThan' | 'Equals' | 'GreaterThanOrEqual' | 'LessThanOrEqual';
  threshold: number;
  severity: 'Info' | 'Warning' | 'Critical';
  enabled: boolean;
  cooldown_seconds: number;
}

interface SimpleAlertRule {
  id: string;
  metric: 'CpuUsage' | 'MemoryUsage' | 'DiskUsage';
  operator: 'GreaterThan' | 'LessThan' | 'Equals' | 'GreaterThanOrEqual' | 'LessThanOrEqual';
  threshold: number;
  severity: 'Info' | 'Warning' | 'Critical';
  enabled: boolean;
  cooldown_seconds: number;
}

const OPERATOR_LABELS: Record<string, string> = {
  GreaterThan: '>',
  LessThan: '<',
  Equals: '=',
  GreaterThanOrEqual: '≥',
  LessThanOrEqual: '≤'
};

const METRIC_LABELS: Record<string, string> = {
  CpuUsage: 'CPU Usage',
  MemoryUsage: 'Memory Usage',
  DiskUsage: 'Disk Usage'
};

const AlertRules: React.FC = () => {
  const [rules, setRules] = useState<SimpleAlertRule[]>([]);
  const [isEditing, setIsEditing] = useState<string | null>(null);
  const [newRule, setNewRule] = useState<Partial<SimpleAlertRule>>({
    metric: 'CpuUsage',
    operator: 'GreaterThan',
    threshold: 80,
    severity: 'Warning',
    enabled: true,
    cooldown_seconds: 300
  });

  useEffect(() => {
    loadRules();
  }, []);

  const loadRules = async () => {
    try {
      const backendRules = await invoke<AlertRule[]>('get_alert_rules');
      const simpleRules = backendRules.map(convertToSimpleRule);
      setRules(simpleRules);
    } catch (err) {
      console.error('Failed to load alert rules:', err);
    }
  };

  const convertToSimpleRule = (rule: AlertRule): SimpleAlertRule => ({
    id: rule.id,
    metric: rule.metric,
    operator: rule.operator,
    threshold: rule.threshold,
    severity: rule.severity,
    enabled: rule.enabled,
    cooldown_seconds: rule.cooldown_seconds
  });

  const convertToBackendRule = (rule: SimpleAlertRule): AlertRule => ({
    id: rule.id,
    metric: rule.metric,
    operator: rule.operator,
    threshold: rule.threshold,
    severity: rule.severity,
    enabled: rule.enabled,
    cooldown_seconds: rule.cooldown_seconds
  });

  const toggleRule = async (id: string) => {
    const rule = rules.find(r => r.id === id);
    if (!rule) return;

    const updatedRule = { ...rule, enabled: !rule.enabled };
    setRules(prev => prev.map(r => r.id === id ? updatedRule : r));

    try {
      // add_alert_rule upserts by id (retain(id != new.id) then push) on the backend,
      // so a single call is an atomic replace. Doing remove-then-add here left a window
      // where the backend rule was deleted but the re-add hadn't succeeded yet.
      await invoke('add_alert_rule', { rule: convertToBackendRule(updatedRule) });
    } catch (err) {
      console.error('Failed to toggle rule:', err);
      setRules(prev => prev.map(r => r.id === id ? rule : r)); // Revert on error
    }
  };

  const deleteRule = async (id: string) => {
    setRules(prev => prev.filter(rule => rule.id !== id));
    try {
      await invoke('remove_alert_rule', { ruleId: id });
    } catch (err) {
      console.error('Failed to delete rule:', err);
      loadRules(); // Reload on error
    }
  };

  const addRule = async () => {
    if (newRule.threshold === undefined) return;
    
    const rule: SimpleAlertRule = {
      id: Date.now().toString(),
      metric: newRule.metric as 'CpuUsage' | 'MemoryUsage' | 'DiskUsage',
      operator: newRule.operator as 'GreaterThan' | 'LessThan' | 'Equals' | 'GreaterThanOrEqual' | 'LessThanOrEqual',
      threshold: newRule.threshold,
      severity: newRule.severity as 'Info' | 'Warning' | 'Critical',
      enabled: true,
      cooldown_seconds: newRule.cooldown_seconds || 300
    };
    
    setRules(prev => [...prev, rule]);
    
    try {
      await invoke('add_alert_rule', { rule: convertToBackendRule(rule) });
    } catch (err) {
      console.error('Failed to add rule:', err);
      setRules(prev => prev.filter(r => r.id !== rule.id)); // Remove on error
    }
    
    setNewRule({
      metric: 'CpuUsage',
      operator: 'GreaterThan',
      threshold: 80,
      severity: 'Warning',
      enabled: true,
      cooldown_seconds: 300
    });
    setIsEditing(null);
  };

  return (
    <div className="bg-bg-card border border-gray-800 rounded-xl overflow-hidden shadow-sm">
      <div className="p-4 border-b border-gray-800 flex justify-between items-center bg-bg-card/50">
        <div className="flex items-center space-x-2">
          <Bell className="h-4 w-4 text-accent" />
          <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest">Alert Rules</h3>
        </div>
        <button
          onClick={() => setIsEditing('new')}
          className="flex items-center space-x-1 text-[10px] font-bold text-accent hover:text-accent/80 uppercase tracking-wider transition-colors"
        >
          <Plus className="h-3 w-3" />
          <span>Add Rule</span>
        </button>
      </div>

      <div className="divide-y divide-gray-800/50">
        {rules.map((rule) => (
          <div
            key={rule.id}
            className={cn(
              "p-3 flex items-center justify-between transition-colors",
              rule.enabled ? "hover:bg-white/5" : "opacity-50 bg-black/20"
            )}
          >
            <div className="flex items-center space-x-3">
              <button
                role="switch"
                aria-checked={rule.enabled}
                aria-label={`Toggle alert rule: ${METRIC_LABELS[rule.metric]} ${OPERATOR_LABELS[rule.operator]} ${rule.threshold}%`}
                onClick={() => toggleRule(rule.id)}
                className={cn(
                  "w-8 h-4 rounded-full transition-colors relative",
                  rule.enabled ? "bg-accent" : "bg-gray-700"
                )}
              >
                <div className={cn(
                  "w-3 h-3 rounded-full bg-white absolute top-0.5 transition-all",
                  rule.enabled ? "left-4.5" : "left-0.5"
                )} />
              </button>
              
              <div>
                <div className="flex items-center space-x-2">
                  <span className="text-xs font-medium text-gray-300">
                    {METRIC_LABELS[rule.metric]}
                  </span>
                  <span className="text-xs text-gray-500">
                    {OPERATOR_LABELS[rule.operator]}
                  </span>
                  <span className="text-xs font-mono font-bold text-gray-300">
                    {rule.threshold}%
                  </span>
                </div>
                <div className="flex items-center space-x-1 mt-0.5">
                  <span className={cn(
                    "text-[10px] font-bold uppercase px-1.5 py-0.5 rounded",
                    rule.severity === 'Critical' ? "bg-alert/20 text-alert" : 
                    rule.severity === 'Warning' ? "bg-warning/20 text-warning" : "bg-blue-500/20 text-blue-400"
                  )}>
                    {rule.severity}
                  </span>
                  <span className={cn(
                    "text-[10px] font-bold uppercase px-1.5 py-0.5 rounded",
                    rule.severity === 'Critical' ? "text-alert" : 
                    rule.severity === 'Warning' ? "text-warning" : "text-blue-400"
                  )}>
                    {OPERATOR_LABELS[rule.operator]} {rule.threshold}%
                  </span>
                </div>
              </div>
            </div>

            <button
              onClick={() => deleteRule(rule.id)}
              className="p-1.5 hover:bg-red-500/10 rounded text-gray-500 hover:text-red-400 transition-colors"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}

        {isEditing === 'new' && (
          <div className="p-3 bg-accent/5 border-l-4 border-accent">
            <div className="grid grid-cols-4 gap-2 mb-3">
              <select
                value={newRule.metric}
                onChange={(e) => setNewRule(prev => ({ ...prev, metric: e.target.value as SimpleAlertRule['metric'] }))}
                className="bg-zinc-900 border border-gray-800 rounded px-2 py-1 text-xs text-gray-300"
              >
                <option value="CpuUsage">CPU</option>
                <option value="MemoryUsage">Memory</option>
                <option value="DiskUsage">Disk</option>
              </select>
              
              <select
                value={newRule.operator}
                onChange={(e) => setNewRule(prev => ({ ...prev, operator: e.target.value as SimpleAlertRule['operator'] }))}
                className="bg-zinc-900 border border-gray-800 rounded px-2 py-1 text-xs text-gray-300"
              >
                <option value="GreaterThan">&gt;</option>
                <option value="LessThan">&lt;</option>
                <option value="GreaterThanOrEqual">≥</option>
                <option value="LessThanOrEqual">≤</option>
                <option value="Equals">=</option>
              </select>
              
              <input
                type="number"
                value={newRule.threshold}
                onChange={(e) => setNewRule(prev => ({ ...prev, threshold: Number(e.target.value) }))}
                className="bg-zinc-900 border border-gray-800 rounded px-2 py-1 text-xs text-gray-300"
                placeholder="Threshold %"
                min="0"
                max="100"
              />
              
              <select
                value={newRule.severity}
                onChange={(e) => setNewRule(prev => ({ ...prev, severity: e.target.value as SimpleAlertRule['severity'] }))}
                className="bg-zinc-900 border border-gray-800 rounded px-2 py-1 text-xs text-gray-300"
              >
                <option value="Info">Info</option>
                <option value="Warning">Warning</option>
                <option value="Critical">Critical</option>
              </select>
            </div>
            
            <div className="flex justify-end space-x-2">
              <button
                onClick={() => setIsEditing(null)}
                className="px-3 py-1 text-[10px] text-gray-500 hover:text-gray-300 uppercase"
              >
                Cancel
              </button>
              <button
                onClick={addRule}
                className="px-3 py-1 bg-accent text-black text-[10px] font-bold uppercase rounded hover:bg-accent/90 flex items-center space-x-1"
              >
                <Check className="h-3 w-3" />
                <span>Save</span>
              </button>
            </div>
          </div>
        )}

        {rules.length === 0 && !isEditing && (
          <div className="p-8 text-center text-gray-500">
            <Bell className="h-8 w-8 mx-auto mb-2 opacity-30" />
            <p className="text-sm">No alert rules configured</p>
            <p className="text-[10px] mt-1">Add a rule to get notified</p>
          </div>
        )}
      </div>
    </div>
  );
};

export default AlertRules;
