import React, { useState } from 'react';
import { Settings, Tag, X } from 'lucide-react';

interface WorkflowSettingsProps {
  settings: any;
  tags: string[];
  metadata: {
    category?: string;
    author?: string;
    version?: string;
  };
  onSettingsChange: (settings: any) => void;
  onTagsChange: (tags: string[]) => void;
  onMetadataChange: (metadata: any) => void;
}

const WorkflowSettings: React.FC<WorkflowSettingsProps> = ({
  settings,
  tags,
  metadata,
  onSettingsChange,
  onTagsChange,
  onMetadataChange,
}) => {
  const [newTag, setNewTag] = useState('');
  const [isExpanded, setIsExpanded] = useState(false);

  const handleAddTag = () => {
    if (newTag.trim() && !tags.includes(newTag.trim())) {
      onTagsChange([...tags, newTag.trim()]);
      setNewTag('');
    }
  };

  const handleRemoveTag = (tagToRemove: string) => {
    onTagsChange(tags.filter(t => t !== tagToRemove));
  };

  return (
    <div className="bg-gray-800 border border-gray-700 rounded-lg p-4 mb-4">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex items-center justify-between w-full text-left"
      >
        <div className="flex items-center gap-2">
          <Settings className="w-5 h-5 text-blue-400" />
          <h3 className="text-lg font-semibold text-white">Workflow Settings</h3>
        </div>
        <span className="text-gray-400">{isExpanded ? '▼' : '▶'}</span>
      </button>

      {isExpanded && (
        <div className="mt-4 space-y-4">
          {/* Execution Settings */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-gray-300">Execution Settings</h4>
            
            <div>
              <label className="block text-sm text-gray-400 mb-1">Timezone</label>
              <input
                type="text"
                value={settings.timezone || 'UTC'}
                onChange={(e) => onSettingsChange({ ...settings, timezone: e.target.value })}
                className="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded text-white text-sm"
                placeholder="UTC"
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">Execution Timeout (seconds)</label>
              <input
                type="number"
                value={settings.execution_timeout || 300}
                onChange={(e) => onSettingsChange({ ...settings, execution_timeout: parseInt(e.target.value) })}
                className="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded text-white text-sm"
                placeholder="300"
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">Execution Order</label>
              <select
                value={settings.execution_order || 'v1'}
                onChange={(e) => onSettingsChange({ ...settings, execution_order: e.target.value })}
                className="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded text-white text-sm"
              >
                <option value="v0">v0 (Legacy)</option>
                <option value="v1">v1 (Optimized)</option>
              </select>
            </div>
          </div>

          {/* Data Saving */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-gray-300">Data Saving</h4>
            
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={settings.save_execution_progress !== false}
                onChange={(e) => onSettingsChange({ ...settings, save_execution_progress: e.target.checked })}
                className="rounded bg-gray-700 border-gray-600"
              />
              <span className="text-sm text-gray-300">Save execution progress</span>
            </label>

            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={settings.save_manual_executions !== false}
                onChange={(e) => onSettingsChange({ ...settings, save_manual_executions: e.target.checked })}
                className="rounded bg-gray-700 border-gray-600"
              />
              <span className="text-sm text-gray-300">Save manual executions</span>
            </label>
          </div>

          {/* Tags */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-gray-300 flex items-center gap-2">
              <Tag className="w-4 h-4" />
              Tags
            </h4>
            
            <div className="flex gap-2">
              <input
                type="text"
                value={newTag}
                onChange={(e) => setNewTag(e.target.value)}
                onKeyPress={(e) => e.key === 'Enter' && handleAddTag()}
                className="flex-1 px-3 py-2 bg-gray-700 border border-gray-600 rounded text-white text-sm"
                placeholder="Add tag..."
              />
              <button
                onClick={handleAddTag}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm"
              >
                Add
              </button>
            </div>

            <div className="flex flex-wrap gap-2">
              {tags.map(tag => (
                <span
                  key={tag}
                  className="inline-flex items-center gap-1 px-3 py-1 bg-blue-600/20 border border-blue-500/30 rounded-full text-sm text-blue-300"
                >
                  {tag}
                  <button
                    onClick={() => handleRemoveTag(tag)}
                    className="hover:text-blue-100"
                  >
                    <X className="w-3 h-3" />
                  </button>
                </span>
              ))}
            </div>
          </div>

          {/* Metadata */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-gray-300">Metadata</h4>
            
            <div>
              <label className="block text-sm text-gray-400 mb-1">Category</label>
              <input
                type="text"
                value={metadata.category || ''}
                onChange={(e) => onMetadataChange({ ...metadata, category: e.target.value })}
                className="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded text-white text-sm"
                placeholder="e.g., Infrastructure, Monitoring"
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">Author</label>
              <input
                type="text"
                value={metadata.author || ''}
                onChange={(e) => onMetadataChange({ ...metadata, author: e.target.value })}
                className="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded text-white text-sm"
                placeholder="e.g., DevOps Team"
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">Version</label>
              <input
                type="text"
                value={metadata.version || ''}
                onChange={(e) => onMetadataChange({ ...metadata, version: e.target.value })}
                className="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded text-white text-sm"
                placeholder="e.g., 1.0.0"
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default WorkflowSettings;
