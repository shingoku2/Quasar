import React, { useState } from 'react';
import { Bell, Plus, Trash2, AlertTriangle, Check } from 'lucide-react';
import { cn } from '../../lib/utils';
import { invoke } from '@tauri-apps/api/core';

export interface AlertRule {
  id: string;
  metric: 'cpu' | 'memory' | 'disk';
  operator: 'gt' | 'lt' | 'eq' | 'gte' | 'lte';
  threshold: number;
  severity: 'critical' | 'warning' | 'info';
  enabled: boolean;
}

const OPERATOR_LABELS: Record<string, string> = {
  gt: '>',
  lt: '<',
  eq: '=',
  gte: '≥',
  lte: '≤'
};

const METRIC_LABELS: Record<string, string> = {
  cpu: 'CPU Usage',
  memory: 'Memory Usage',
  disk: 'Disk Usage'
};

const AlertRules: React.FC = () => {
  const [rules, setRules] = useState<AlertRule[]>([
    { id: '1', metric: 'cpu', operator: 'gt', threshold: 80, severity: 'warning', enabled: true },
    { id: '2', metric: 'cpu', operator: 'gt', threshold: 95, severity: 'critical', enabled: true },
    { id: '3', metric: 'memory', operator: 'gt', threshold: 85, severity: 'warning', enabled: true },
  ]);
  const [isEditing, setIsEditing] = useState<string | null>(null);
  const [newRule, setNewRule] = useState<Partial<AlertRule>>({
    metric: 'cpu',
    operator: 'gt',
    threshold: 80,
    severity: 'warning',
    enabled: true
  });

  const toggleRule = (id: string) => {
    setRules(prev => prev.map(rule =>
      rule.id === id ? { ...rule, enabled: !rule.enabled } : rule
    ));
  };

  const deleteRule = (id: string) => {
    setRules(prev => prev.filter(rule => rule.id !== id));
    invoke('remove_alert_rule', { ruleId: id }).catch(console.error);
  };

  const addRule = () => {
    if (newRule.threshold === undefined) return;
    
    const rule: AlertRule = {
      id: Date.now().toString(),
      metric: newRule.metric as 'cpu' | 'memory' | 'disk',
      operator: newRule.operator as 'gt' | 'lt' | 'eq' | 'gte' | 'lte',
      threshold: newRule.threshold,
      severity: newRule.severity as 'critical' | 'warning' | 'info',
      enabled: true
    };
    
    setRules(prev => [...prev, rule]);
    invoke('add_alert_rule', { rule }).catch(console.error);
    
    setNewRule({
      metric: 'cpu',
      operator: 'gt',
      threshold: 80,
      severity: 'warning',
      enabled: true
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
                  <AlertTriangle className={cn(
                    "h-3 w-3",
                    rule.severity === 'critical' ? "text-alert" :
                    rule.severity === 'warning' ? "text-warning" : "text-accent"
                  )} />
                  <span className={cn(
                    "text-[10px] uppercase",
                    rule.severity === 'critical' ? "text-alert" :
                    rule.severity === 'warning' ? "text-warning" : "text-accent"
                  )}>
                    {rule.severity}
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
                onChange={(e) => setNewRule(prev => ({ ...prev, metric: e.target.value as any }))}
                className="bg-zinc-900 border border-gray-800 rounded px-2 py-1 text-xs text-gray-300"
              >
                <option value="cpu">CPU</option>
                <option value="memory">Memory</option>
                <option value="disk">Disk</option>
              </select>
              
              <select
                value={newRule.operator}
                onChange={(e) => setNewRule(prev => ({ ...prev, operator: e.target.value as any }))}
                className="bg-zinc-900 border border-gray-800 rounded px-2 py-1 text-xs text-gray-300"
              >
                <option value="gt">&gt;</option>
                <option value="lt">&lt;</option>
                <option value="gte">≥</option>
                <option value="lte">≤</option>
                <option value="eq">=</option>
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
                onChange={(e) => setNewRule(prev => ({ ...prev, severity: e.target.value as any }))}
                className="bg-zinc-900 border border-gray-800 rounded px-2 py-1 text-xs text-gray-300"
              >
                <option value="info">Info</option>
                <option value="warning">Warning</option>
                <option value="critical">Critical</option>
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
