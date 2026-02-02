import React, { useState } from 'react';
import { Search, X, Zap, Play, GitBranch, Bell, Database, Globe, Mail, MessageSquare, Calendar, FileText } from 'lucide-react';

interface NodeLibraryProps {
  onAddNode: (type: string, position: { x: number; y: number }) => void;
  onClose: () => void;
}

const NodeLibrary: React.FC<NodeLibraryProps> = ({ onAddNode, onClose }) => {
  const [searchTerm, setSearchTerm] = useState('');

  const nodeCategories = [
    {
      name: 'Core',
      nodes: [
        { type: 'trigger', label: 'Manual Trigger', icon: Zap, description: 'Start workflow manually' },
        { type: 'action', label: 'SSH Command', icon: Play, description: 'Execute SSH commands' },
        { type: 'condition', label: 'IF Condition', icon: GitBranch, description: 'Conditional branching' },
        { type: 'notification', label: 'Notification', icon: Bell, description: 'Send notifications' },
      ]
    },
    {
      name: 'Triggers',
      nodes: [
        { type: 'trigger', label: 'Webhook', icon: Globe, description: 'Trigger via HTTP webhook' },
        { type: 'trigger', label: 'Schedule', icon: Calendar, description: 'Run on schedule' },
        { type: 'trigger', label: 'Manual', icon: Zap, description: 'Start manually' },
      ]
    },
    {
      name: 'Actions',
      nodes: [
        { type: 'action', label: 'HTTP Request', icon: Globe, description: 'Make HTTP requests' },
        { type: 'action', label: 'Database Query', icon: Database, description: 'Query databases' },
        { type: 'action', label: 'Send Email', icon: Mail, description: 'Send email messages' },
        { type: 'action', label: 'Slack Message', icon: MessageSquare, description: 'Post to Slack' },
      ]
    },
  ];

  const filteredCategories = nodeCategories.map(category => ({
    ...category,
    nodes: category.nodes.filter(node =>
      node.label.toLowerCase().includes(searchTerm.toLowerCase()) ||
      node.description.toLowerCase().includes(searchTerm.toLowerCase())
    )
  })).filter(category => category.nodes.length > 0);

  const handleAddNode = (type: string) => {
    // Add node at center of viewport
    onAddNode(type as 'trigger' | 'action' | 'condition' | 'notification', { x: 400, y: 300 });
    onClose();
  };

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm z-50 flex items-center justify-center">
      <div className="bg-gray-900 border border-gray-700 rounded-lg shadow-2xl w-[600px] max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="p-4 border-b border-gray-700">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-xl font-semibold text-white">Add Node</h2>
            <button
              onClick={onClose}
              className="p-1 hover:bg-gray-800 rounded text-gray-400 hover:text-white"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
          
          {/* Search */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder="Search nodes..."
              className="w-full pl-10 pr-4 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white placeholder-gray-400 focus:outline-none focus:border-blue-500"
              autoFocus
            />
          </div>
        </div>

        {/* Node List */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {filteredCategories.length === 0 ? (
            <div className="text-center py-8 text-gray-400">
              No nodes found matching "{searchTerm}"
            </div>
          ) : (
            filteredCategories.map(category => (
              <div key={category.name}>
                <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider mb-2">
                  {category.name}
                </h3>
                <div className="space-y-1">
                  {category.nodes.map((node, idx) => {
                    const Icon = node.icon;
                    return (
                      <button
                        key={`${node.type}-${idx}`}
                        onClick={() => handleAddNode(node.type)}
                        className="w-full flex items-start gap-3 p-3 rounded-lg hover:bg-gray-800 transition-colors text-left group"
                      >
                        <div className="p-2 rounded-lg bg-blue-500/10 group-hover:bg-blue-500/20 transition-colors">
                          <Icon className="w-5 h-5 text-blue-400" />
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="text-white font-medium">{node.label}</div>
                          <div className="text-sm text-gray-400">{node.description}</div>
                        </div>
                      </button>
                    );
                  })}
                </div>
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-gray-700 text-xs text-gray-400">
          Press <kbd className="px-1.5 py-0.5 bg-gray-800 rounded">Tab</kbd> to add node and keep dialog open
        </div>
      </div>
    </div>
  );
};

export default NodeLibrary;
