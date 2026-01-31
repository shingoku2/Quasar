import React, { useState, useEffect } from 'react';
import { X, Settings, Terminal, Clock, Globe, Bell, Monitor, MessageSquare, GitBranch, FileUp } from 'lucide-react';
import { NodeData } from './Canvas';

interface PropertiesPanelProps {
  node: NodeData | null;
  onClose: () => void;
  onUpdate: (nodeId: string, data: Partial<NodeData['data']>) => void;
}

const PropertiesPanel: React.FC<PropertiesPanelProps> = ({ node, onClose, onUpdate }) => {
  const [label, setLabel] = useState('');
  const [config, setConfig] = useState<Record<string, any>>({});

  useEffect(() => {
    if (node) {
      setLabel(node.data.label);
      setConfig(node.data.config || {});
    }
  }, [node]);

  if (!node) return null;

  const handleLabelChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setLabel(e.target.value);
    onUpdate(node.id, { label: e.target.value });
  };

  const handleConfigChange = (key: string, value: any) => {
    const newConfig = { ...config, [key]: value };
    setConfig(newConfig);
    onUpdate(node.id, { config: newConfig });
  };

  const renderTriggerConfig = () => (
    <div className="space-y-4">
      <div>
        <label className="text-xs font-medium text-gray-400 uppercase">Trigger Type</label>
        <select
          value={config.triggerType || 'manual'}
          onChange={(e) => handleConfigChange('triggerType', e.target.value)}
          className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
        >
          <option value="manual">Manual</option>
          <option value="scheduled">Scheduled</option>
          <option value="webhook">Webhook</option>
        </select>
      </div>
      {config.triggerType === 'scheduled' && (
        <div>
          <label className="text-xs font-medium text-gray-400 uppercase">Cron Expression</label>
          <input
            type="text"
            value={config.cron || '* * * * *'}
            onChange={(e) => handleConfigChange('cron', e.target.value)}
            placeholder="* * * * *"
            className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm font-mono"
          />
          <p className="text-xs text-gray-500 mt-1">Format: minute hour day month weekday</p>
        </div>
      )}
      {config.triggerType === 'webhook' && (
        <div>
          <label className="text-xs font-medium text-gray-400 uppercase">Webhook Path</label>
          <input
            type="text"
            value={config.webhookPath || '/webhook/' + node.id}
            onChange={(e) => handleConfigChange('webhookPath', e.target.value)}
            className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm font-mono"
          />
        </div>
      )}
    </div>
  );

  const renderActionConfig = () => (
    <div className="space-y-4">
      <div>
        <label className="text-xs font-medium text-gray-400 uppercase">Action Type</label>
        <select
          value={config.actionType || 'ssh'}
          onChange={(e) => handleConfigChange('actionType', e.target.value)}
          className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
        >
          <option value="ssh">SSH Command</option>
          <option value="file_transfer">File Transfer</option>
        </select>
      </div>
      {config.actionType === 'ssh' ? (
        <>
          <div>
            <label className="text-xs font-medium text-gray-400 uppercase">Host</label>
            <input
              type="text"
              value={config.host || ''}
              onChange={(e) => handleConfigChange('host', e.target.value)}
              placeholder="192.168.1.1"
              className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="text-xs font-medium text-gray-400 uppercase">Port</label>
            <input
              type="number"
              value={config.port || 22}
              onChange={(e) => handleConfigChange('port', parseInt(e.target.value))}
              className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="text-xs font-medium text-gray-400 uppercase">Username</label>
            <input
              type="text"
              value={config.username || ''}
              onChange={(e) => handleConfigChange('username', e.target.value)}
              className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="text-xs font-medium text-gray-400 uppercase">Command</label>
            <textarea
              value={config.command || ''}
              onChange={(e) => handleConfigChange('command', e.target.value)}
              placeholder="uptime"
              rows={3}
              className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm font-mono"
            />
          </div>
        </>
      ) : (
        <>
          <div>
            <label className="text-xs font-medium text-gray-400 uppercase">Source</label>
            <input
              type="text"
              value={config.source || ''}
              onChange={(e) => handleConfigChange('source', e.target.value)}
              placeholder="/local/path"
              className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="text-xs font-medium text-gray-400 uppercase">Destination</label>
            <input
              type="text"
              value={config.destination || ''}
              onChange={(e) => handleConfigChange('destination', e.target.value)}
              placeholder="/remote/path"
              className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="text-xs font-medium text-gray-400 uppercase">Direction</label>
            <select
              value={config.direction || 'upload'}
              onChange={(e) => handleConfigChange('direction', e.target.value)}
              className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
            >
              <option value="upload">Upload</option>
              <option value="download">Download</option>
            </select>
          </div>
        </>
      )}
    </div>
  );

  const renderConditionConfig = () => (
    <div className="space-y-4">
      <div>
        <label className="text-xs font-medium text-gray-400 uppercase">Expression</label>
        <textarea
          value={config.expression || ''}
          onChange={(e) => handleConfigChange('expression', e.target.value)}
          placeholder="${'{'}variable{'}'} == 'value'"
          rows={4}
          className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm font-mono"
        />
        <p className="text-xs text-gray-500 mt-1">Use ${'{'}variable{'}'} to reference variables</p>
      </div>
    </div>
  );

  const renderNotificationConfig = () => (
    <div className="space-y-4">
      <div>
        <label className="text-xs font-medium text-gray-400 uppercase">Channel</label>
        <select
          value={config.channel || 'in_app'}
          onChange={(e) => handleConfigChange('channel', e.target.value)}
          className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
        >
          <option value="in_app">In-App Only</option>
          <option value="system">System Only</option>
          <option value="both">Both</option>
        </select>
      </div>
      <div>
        <label className="text-xs font-medium text-gray-400 uppercase">Title</label>
        <input
          type="text"
          value={config.title || ''}
          onChange={(e) => handleConfigChange('title', e.target.value)}
          placeholder="Notification Title"
          className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
        />
      </div>
      <div>
        <label className="text-xs font-medium text-gray-400 uppercase">Message</label>
        <textarea
          value={config.message || ''}
          onChange={(e) => handleConfigChange('message', e.target.value)}
          placeholder="Notification message..."
          rows={4}
          className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
        />
      </div>
    </div>
  );

  const getNodeIcon = () => {
    switch (node.type) {
      case 'trigger':
        const triggerType = config.triggerType || 'manual';
        if (triggerType === 'scheduled') return <Clock className="w-5 h-5" />;
        if (triggerType === 'webhook') return <Globe className="w-5 h-5" />;
        return <Terminal className="w-5 h-5" />;
      case 'action':
        const actionType = config.actionType || 'ssh';
        if (actionType === 'file_transfer') return <FileUp className="w-5 h-5" />;
        return <Terminal className="w-5 h-5" />;
      case 'condition':
        return <GitBranch className="w-5 h-5" />;
      case 'notification':
        const channel = config.channel || 'in_app';
        if (channel === 'system') return <Monitor className="w-5 h-5" />;
        if (channel === 'both') return <MessageSquare className="w-5 h-5" />;
        return <Bell className="w-5 h-5" />;
      default:
        return <Settings className="w-5 h-5" />;
    }
  };

  return (
    <div className="w-80 border-l border-gray-800 bg-bg-card flex flex-col h-full">
      {/* Header */}
      <div className="h-14 border-b border-gray-800 flex items-center justify-between px-4">
        <div className="flex items-center space-x-2">
          <div className="text-accent">{getNodeIcon()}</div>
          <h3 className="font-bold text-sm">{node.type.charAt(0).toUpperCase() + node.type.slice(1)} Properties</h3>
        </div>
        <button onClick={onClose} className="text-gray-400 hover:text-white">
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-6">
        {/* Common Label */}
        <div>
          <label className="text-xs font-medium text-gray-400 uppercase">Label</label>
          <input
            type="text"
            value={label}
            onChange={handleLabelChange}
            className="mt-1 w-full bg-zinc-900 border border-gray-700 rounded px-3 py-2 text-sm"
          />
        </div>

        {/* Type-specific config */}
        {node.type === 'trigger' && renderTriggerConfig()}
        {node.type === 'action' && renderActionConfig()}
        {node.type === 'condition' && renderConditionConfig()}
        {node.type === 'notification' && renderNotificationConfig()}
      </div>

      {/* Footer */}
      <div className="p-4 border-t border-gray-800">
        <div className="text-xs text-gray-500">
          Node ID: <code className="text-accent font-mono">{node.id}</code>
        </div>
      </div>
    </div>
  );
};

export default PropertiesPanel;
