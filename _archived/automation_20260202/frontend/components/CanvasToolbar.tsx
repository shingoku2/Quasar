import React from 'react';
import { Play, Save, Download, Upload, Settings, Maximize2, ZoomIn, ZoomOut, Undo, Redo, Copy, Trash2 } from 'lucide-react';

interface CanvasToolbarProps {
  workflowName: string;
  onWorkflowNameChange: (name: string) => void;
  onExecute: () => void;
  onSave: () => void;
  onExport: () => void;
  onImport: () => void;
  onSettings: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomFit: () => void;
  onUndo?: () => void;
  onRedo?: () => void;
  canUndo?: boolean;
  canRedo?: boolean;
  zoom: number;
  isExecuting?: boolean;
}

const CanvasToolbar: React.FC<CanvasToolbarProps> = ({
  workflowName,
  onWorkflowNameChange,
  onExecute,
  onSave,
  onExport,
  onImport,
  onSettings,
  onZoomIn,
  onZoomOut,
  onZoomFit,
  onUndo,
  onRedo,
  canUndo,
  canRedo,
  zoom,
  isExecuting,
}) => {
  return (
    <div className="h-14 bg-gray-900 border-b border-gray-800 flex items-center justify-between px-4">
      {/* Left Section - Workflow Name */}
      <div className="flex items-center gap-3">
        <input
          type="text"
          value={workflowName}
          onChange={(e) => onWorkflowNameChange(e.target.value)}
          className="bg-transparent text-white text-lg font-medium focus:outline-none focus:bg-gray-800 px-2 py-1 rounded"
          placeholder="Untitled Workflow"
        />
        <span className="text-xs text-gray-500">Saved</span>
      </div>

      {/* Center Section - Main Actions */}
      <div className="flex items-center gap-2">
        {/* Undo/Redo */}
        <div className="flex items-center gap-1 border-r border-gray-700 pr-2 mr-2">
          <button
            onClick={onUndo}
            disabled={!canUndo}
            className="p-2 hover:bg-gray-800 rounded text-gray-400 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
            title="Undo"
          >
            <Undo className="w-4 h-4" />
          </button>
          <button
            onClick={onRedo}
            disabled={!canRedo}
            className="p-2 hover:bg-gray-800 rounded text-gray-400 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
            title="Redo"
          >
            <Redo className="w-4 h-4" />
          </button>
        </div>

        {/* Zoom Controls */}
        <div className="flex items-center gap-1 border-r border-gray-700 pr-2 mr-2">
          <button
            onClick={onZoomOut}
            className="p-2 hover:bg-gray-800 rounded text-gray-400 hover:text-white"
            title="Zoom Out"
          >
            <ZoomOut className="w-4 h-4" />
          </button>
          <span className="text-sm text-gray-400 min-w-[3rem] text-center">
            {Math.round(zoom * 100)}%
          </span>
          <button
            onClick={onZoomIn}
            className="p-2 hover:bg-gray-800 rounded text-gray-400 hover:text-white"
            title="Zoom In"
          >
            <ZoomIn className="w-4 h-4" />
          </button>
          <button
            onClick={onZoomFit}
            className="p-2 hover:bg-gray-800 rounded text-gray-400 hover:text-white"
            title="Fit to Screen"
          >
            <Maximize2 className="w-4 h-4" />
          </button>
        </div>

        {/* File Operations */}
        <div className="flex items-center gap-1 border-r border-gray-700 pr-2 mr-2">
          <button
            onClick={onImport}
            className="p-2 hover:bg-gray-800 rounded text-gray-400 hover:text-white"
            title="Import Workflow"
          >
            <Upload className="w-4 h-4" />
          </button>
          <button
            onClick={onExport}
            className="p-2 hover:bg-gray-800 rounded text-gray-400 hover:text-white"
            title="Export Workflow"
          >
            <Download className="w-4 h-4" />
          </button>
        </div>

        {/* Settings */}
        <button
          onClick={onSettings}
          className="p-2 hover:bg-gray-800 rounded text-gray-400 hover:text-white"
          title="Workflow Settings"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>

      {/* Right Section - Execute & Save */}
      <div className="flex items-center gap-2">
        <button
          onClick={onSave}
          className="px-4 py-2 bg-gray-800 hover:bg-gray-700 text-white rounded-lg flex items-center gap-2 text-sm font-medium"
        >
          <Save className="w-4 h-4" />
          Save
        </button>
        <button
          onClick={onExecute}
          disabled={isExecuting}
          className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-600/50 text-white rounded-lg flex items-center gap-2 text-sm font-medium"
        >
          <Play className="w-4 h-4" />
          {isExecuting ? 'Executing...' : 'Execute'}
        </button>
      </div>
    </div>
  );
};

export default CanvasToolbar;
