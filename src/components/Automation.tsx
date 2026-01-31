import React, { useState } from 'react';
import { Workflow, Plus, FolderOpen, Settings } from 'lucide-react';
import Canvas from './automation/Canvas';

const Automation: React.FC = () => {
  const [activeWorkflow, setActiveWorkflow] = useState<string | null>(null);
  const [workflows, setWorkflows] = useState<{ id: string; name: string }[]>([]);

  const createNewWorkflow = async () => {
    // This would call the backend to create a workflow
    const newId = `wf-${Date.now()}`;
    setWorkflows(prev => [...prev, { id: newId, name: `Workflow ${prev.length + 1}` }]);
    setActiveWorkflow(newId);
  };

  return (
    <div className="h-full flex flex-col bg-zinc-950">
      {/* Header */}
      <div className="h-14 border-b border-gray-800 bg-bg-card flex items-center justify-between px-4">
        <div className="flex items-center space-x-3">
          <Workflow className="w-5 h-5 text-accent" />
          <h1 className="text-lg font-bold">Automation Canvas</h1>
          {activeWorkflow && (
            <span className="text-xs text-gray-500 px-2 py-1 bg-gray-800/50 rounded">
              {workflows.find(w => w.id === activeWorkflow)?.name}
            </span>
          )}
        </div>
        
        <div className="flex items-center space-x-2">
          <button
            onClick={createNewWorkflow}
            className="flex items-center space-x-2 px-3 py-1.5 bg-accent text-black rounded text-sm font-bold hover:bg-accent/90"
          >
            <Plus className="w-4 h-4" />
            <span>New Workflow</span>
          </button>
          <button className="p-2 text-gray-400 hover:text-white hover:bg-white/10 rounded">
            <FolderOpen className="w-4 h-4" />
          </button>
          <button className="p-2 text-gray-400 hover:text-white hover:bg-white/10 rounded">
            <Settings className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Main Canvas Area */}
      <div className="flex-1 overflow-hidden">
        {activeWorkflow ? (
          <Canvas workflowId={activeWorkflow} />
        ) : (
          <div className="h-full flex items-center justify-center">
            <div className="text-center space-y-4">
              <Workflow className="w-16 h-16 text-gray-600 mx-auto" />
              <h2 className="text-xl font-bold text-gray-400">No Workflow Selected</h2>
              <p className="text-sm text-gray-500 max-w-sm">
                Create a new workflow or open an existing one to start building your automation
              </p>
              <button
                onClick={createNewWorkflow}
                className="flex items-center space-x-2 px-4 py-2 bg-accent text-black rounded font-bold hover:bg-accent/90 mx-auto"
              >
                <Plus className="w-4 h-4" />
                <span>Create Your First Workflow</span>
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default Automation;
