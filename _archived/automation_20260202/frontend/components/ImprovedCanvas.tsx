import React, { useState, useRef, useCallback, useEffect } from 'react';
import { Plus } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import TriggerNode from './nodes/TriggerNode';
import ActionNode from './nodes/ActionNode';
import ConditionNode from './nodes/ConditionNode';
import NotificationNode from './nodes/NotificationNode';
import PropertiesPanel from './PropertiesPanel';
import WorkflowSettings from './WorkflowSettings';
import NodeLibrary from './NodeLibrary';
import CanvasToolbar from './CanvasToolbar';

export interface NodeData {
  id: string;
  type: 'trigger' | 'action' | 'condition' | 'notification';
  position: { x: number; y: number };
  data: {
    label: string;
    config?: Record<string, any>;
    typeVersion?: number;
    disabled?: boolean;
    notes?: string;
    notesInFlow?: boolean;
    onError?: 'stop_workflow' | 'continue_regular_output' | 'continue_error_output';
    continueOnFail?: boolean;
    retryOnFail?: boolean;
    maxTries?: number;
    waitBetweenTries?: number;
    credentials?: Record<string, { id?: string; name: string }>;
    parameters?: any;
    alwaysOutputData?: boolean;
  };
}

export interface ConnectionData {
  id: string;
  source: string;
  target: string;
  sourcePort?: string;
  targetPort?: string;
  sourceIndex?: number;
  targetIndex?: number;
  connectionType?: 'main' | 'error' | 'ai' | string;
}

interface ImprovedCanvasProps {
  workflowId?: string;
  initialNodes?: NodeData[];
  initialConnections?: ConnectionData[];
  onSave?: (nodes: NodeData[], connections: ConnectionData[]) => void;
}

const GRID_SIZE = 20;
const MIN_ZOOM = 0.25;
const MAX_ZOOM = 2;

const ImprovedCanvas: React.FC<ImprovedCanvasProps> = ({
  workflowId,
  initialNodes = [],
  initialConnections = [],
  onSave,
}) => {
  const [nodes, setNodes] = useState<NodeData[]>(initialNodes);
  const [connections, setConnections] = useState<ConnectionData[]>(initialConnections);
  const [selectedNodes, setSelectedNodes] = useState<Set<string>>(new Set());
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [isPanning, setIsPanning] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [connectingFrom, setConnectingFrom] = useState<{ nodeId: string; port: string } | null>(null);
  const [tempConnection, setTempConnection] = useState<{ x: number; y: number } | null>(null);
  const [workflowSettings, setWorkflowSettings] = useState<any>({});
  const [workflowTags, setWorkflowTags] = useState<string[]>([]);
  const [workflowMetadata, setWorkflowMetadata] = useState<any>({});
  const [workflowName, setWorkflowName] = useState('Untitled Workflow');
  const [showNodeLibrary, setShowNodeLibrary] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [isExecuting, setIsExecuting] = useState(false);
  
  const canvasRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ nodeId: string; offsetX: number; offsetY: number } | null>(null);

  const gridPattern = `url("data:image/svg+xml,%3Csvg width='${GRID_SIZE}' height='${GRID_SIZE}' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M ${GRID_SIZE} 0 L 0 0 0 ${GRID_SIZE}' fill='none' stroke='%23333' stroke-width='0.5'/%3E%3C/svg%3E")`;

  const snapToGrid = (value: number) => Math.round(value / GRID_SIZE) * GRID_SIZE;

  const selectedNode = selectedNodes.size === 1 
    ? nodes.find(n => selectedNodes.has(n.id)) 
    : undefined;

  const addNode = useCallback((type: NodeData['type'], position: { x: number; y: number }) => {
    const newNode: NodeData = {
      id: `node-${Date.now()}`,
      type,
      position: { x: snapToGrid(position.x), y: snapToGrid(position.y) },
      data: {
        label: `${type.charAt(0).toUpperCase() + type.slice(1)} Node`,
        typeVersion: 1,
        disabled: false,
        onError: 'stop_workflow',
        continueOnFail: false,
        retryOnFail: false,
        maxTries: 3,
        waitBetweenTries: 1000,
        alwaysOutputData: false,
      },
    };
    setNodes(prev => [...prev, newNode]);
  }, []);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      const delta = e.deltaY > 0 ? 0.9 : 1.1;
      setZoom(z => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z * delta)));
    } else {
      setPan(p => ({ x: p.x - e.deltaX, y: p.y - e.deltaY }));
    }
  }, []);

  const handleZoomIn = () => setZoom(z => Math.min(MAX_ZOOM, z * 1.2));
  const handleZoomOut = () => setZoom(z => Math.max(MIN_ZOOM, z / 1.2));
  const handleZoomFit = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };

  const handleExecute = async () => {
    if (!workflowId) return;
    setIsExecuting(true);
    try {
      const execId = await invoke<string>('execute_workflow', { workflowId });
      console.log('Workflow execution started:', execId);
    } catch (err) {
      console.error('Failed to execute workflow:', err);
    } finally {
      setTimeout(() => setIsExecuting(false), 1000);
    }
  };

  const handleSave = async () => {
    if (!workflowId) return;
    // Save logic here
    console.log('Saving workflow...');
  };

  const handleExport = () => {
    const workflow = {
      name: workflowName,
      nodes,
      connections,
      settings: workflowSettings,
      tags: workflowTags,
      metadata: workflowMetadata,
    };
    const blob = new Blob([JSON.stringify(workflow, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${workflowName.replace(/\s+/g, '-').toLowerCase()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleImport = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      const reader = new FileReader();
      reader.onload = (e) => {
        try {
          const workflow = JSON.parse(e.target?.result as string);
          if (workflow.nodes) setNodes(workflow.nodes);
          if (workflow.connections) setConnections(workflow.connections);
          if (workflow.name) setWorkflowName(workflow.name);
          if (workflow.settings) setWorkflowSettings(workflow.settings);
          if (workflow.tags) setWorkflowTags(workflow.tags);
          if (workflow.metadata) setWorkflowMetadata(workflow.metadata);
        } catch (err) {
          console.error('Failed to import workflow:', err);
        }
      };
      reader.readAsText(file);
    };
    input.click();
  };

  const getConnectionColor = (type?: string) => {
    switch (type) {
      case 'error': return '#ef4444';
      case 'ai': return '#a855f7';
      case 'main':
      default: return '#3b82f6';
    }
  };

  const renderConnection = (conn: ConnectionData) => {
    const sourceNode = nodes.find(n => n.id === conn.source);
    const targetNode = nodes.find(n => n.id === conn.target);
    if (!sourceNode || !targetNode) return null;

    const x1 = sourceNode.position.x + 150;
    const y1 = sourceNode.position.y + 40;
    const x2 = targetNode.position.x;
    const y2 = targetNode.position.y + 40;

    const midX = (x1 + x2) / 2;
    const path = `M ${x1} ${y1} C ${midX} ${y1}, ${midX} ${y2}, ${x2} ${y2}`;
    const color = getConnectionColor(conn.connectionType);

    return (
      <g key={conn.id}>
        <path
          d={path}
          fill="none"
          stroke={color}
          strokeWidth={2}
          strokeOpacity={0.6}
          className="hover:stroke-opacity-100 cursor-pointer transition-all"
          onClick={() => setConnections(prev => prev.filter(c => c.id !== conn.id))}
        />
        {conn.connectionType && conn.connectionType !== 'main' && (
          <text
            x={(x1 + x2) / 2}
            y={(y1 + y2) / 2 - 10}
            fill={color}
            fontSize="10"
            textAnchor="middle"
            className="pointer-events-none"
          >
            {conn.connectionType}
          </text>
        )}
      </g>
    );
  };

  const handleUpdateNode = (nodeId: string, updates: Partial<NodeData['data']>) => {
    setNodes(prev => prev.map(n => 
      n.id === nodeId ? { ...n, data: { ...n.data, ...updates } } : n
    ));
  };

  const handleNodeMouseDown = (nodeId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const node = nodes.find(n => n.id === nodeId);
    if (!node) return;

    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return;

    dragRef.current = {
      nodeId,
      offsetX: (e.clientX - rect.left - pan.x) / zoom - node.position.x,
      offsetY: (e.clientY - rect.top - pan.y) / zoom - node.position.y,
    };
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (isPanning) {
      setPan(p => ({ x: p.x + e.movementX, y: p.y + e.movementY }));
    } else if (dragRef.current) {
      const rect = containerRef.current?.getBoundingClientRect();
      if (!rect) return;

      const x = (e.clientX - rect.left - pan.x) / zoom - dragRef.current.offsetX;
      const y = (e.clientY - rect.top - pan.y) / zoom - dragRef.current.offsetY;

      setNodes(prev => prev.map(n =>
        n.id === dragRef.current?.nodeId
          ? { ...n, position: { x: snapToGrid(x), y: snapToGrid(y) } }
          : n
      ));
    }
  };

  const handleMouseUp = () => {
    setIsPanning(false);
    dragRef.current = null;
  };

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button === 1 || (e.button === 0 && e.shiftKey)) {
      setIsPanning(true);
      e.preventDefault();
    } else if (e.button === 0 && e.target === canvasRef.current) {
      setSelectedNodes(new Set());
    }
  };

  const handlePortMouseDown = (nodeId: string) => (e: React.MouseEvent, port: string) => {
    e.stopPropagation();
    setIsConnecting(true);
    setConnectingFrom({ nodeId, port });
  };

  const handlePortMouseUp = (nodeId: string) => (e: React.MouseEvent, port: string) => {
    e.stopPropagation();
    if (isConnecting && connectingFrom && connectingFrom.nodeId !== nodeId) {
      const newConnection: ConnectionData = {
        id: `conn-${Date.now()}`,
        source: connectingFrom.nodeId,
        target: nodeId,
        sourcePort: connectingFrom.port,
        targetPort: port,
        sourceIndex: 0,
        targetIndex: 0,
        connectionType: 'main',
      };
      setConnections(prev => [...prev, newConnection]);
    }
    setIsConnecting(false);
    setConnectingFrom(null);
    setTempConnection(null);
  };

  return (
    <div className="flex flex-col h-full bg-zinc-950">
      {/* Toolbar */}
      <CanvasToolbar
        workflowName={workflowName}
        onWorkflowNameChange={setWorkflowName}
        onExecute={handleExecute}
        onSave={handleSave}
        onExport={handleExport}
        onImport={handleImport}
        onSettings={() => setShowSettings(!showSettings)}
        onZoomIn={handleZoomIn}
        onZoomOut={handleZoomOut}
        onZoomFit={handleZoomFit}
        zoom={zoom}
        isExecuting={isExecuting}
      />

      <div className="flex flex-1 overflow-hidden">
        {/* Canvas */}
        <div ref={containerRef} className="flex-1 relative overflow-hidden">
          <svg
            ref={canvasRef}
            className="w-full h-full"
            style={{
              backgroundImage: gridPattern,
              backgroundSize: `${GRID_SIZE * zoom}px ${GRID_SIZE * zoom}px`,
              backgroundPosition: `${pan.x}px ${pan.y}px`,
            }}
            onWheel={handleWheel}
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
            onMouseLeave={handleMouseUp}
          >
            <g transform={`translate(${pan.x}, ${pan.y}) scale(${zoom})`}>
              {connections.map(renderConnection)}
              
              {nodes.map(node => (
                <foreignObject
                  key={node.id}
                  x={node.position.x}
                  y={node.position.y}
                  width={300}
                  height={80}
                  className="overflow-visible"
                >
                  <div onClick={() => setSelectedNodes(new Set([node.id]))}>
                    {node.type === 'trigger' && (
                      <TriggerNode 
                        node={node} 
                        isSelected={selectedNodes.has(node.id)}
                        onMouseDown={(e: React.MouseEvent) => handleNodeMouseDown(node.id, e)}
                        onPortMouseDown={handlePortMouseDown(node.id)}
                        onPortMouseUp={handlePortMouseUp(node.id)}
                      />
                    )}
                    {node.type === 'action' && (
                      <ActionNode 
                        node={node} 
                        isSelected={selectedNodes.has(node.id)}
                        onMouseDown={(e: React.MouseEvent) => handleNodeMouseDown(node.id, e)}
                        onPortMouseDown={handlePortMouseDown(node.id)}
                        onPortMouseUp={handlePortMouseUp(node.id)}
                      />
                    )}
                    {node.type === 'condition' && (
                      <ConditionNode 
                        node={node} 
                        isSelected={selectedNodes.has(node.id)}
                        onMouseDown={(e: React.MouseEvent) => handleNodeMouseDown(node.id, e)}
                        onPortMouseDown={handlePortMouseDown(node.id)}
                        onPortMouseUp={handlePortMouseUp(node.id)}
                      />
                    )}
                    {node.type === 'notification' && (
                      <NotificationNode 
                        node={node} 
                        isSelected={selectedNodes.has(node.id)}
                        onMouseDown={(e: React.MouseEvent) => handleNodeMouseDown(node.id, e)}
                        onPortMouseDown={handlePortMouseDown(node.id)}
                        onPortMouseUp={handlePortMouseUp(node.id)}
                      />
                    )}
                  </div>
                </foreignObject>
              ))}
            </g>
          </svg>

          {/* Add Node Button (Floating) */}
          <button
            onClick={() => setShowNodeLibrary(true)}
            className="absolute bottom-8 left-8 w-14 h-14 bg-blue-600 hover:bg-blue-700 rounded-full flex items-center justify-center shadow-lg transition-all hover:scale-110"
            title="Add Node"
          >
            <Plus className="w-6 h-6 text-white" />
          </button>

          {/* Status Info */}
          <div className="absolute bottom-4 right-4 bg-gray-900/90 backdrop-blur border border-gray-700 rounded-lg px-4 py-2 text-xs text-gray-400">
            {nodes.length} nodes · {connections.length} connections
            {selectedNodes.size > 0 && ` · ${selectedNodes.size} selected`}
          </div>
        </div>

        {/* Right Sidebar */}
        <div className="w-80 border-l border-gray-800 bg-gray-900 overflow-y-auto">
          {showSettings && (
            <div className="p-4 border-b border-gray-700">
              <WorkflowSettings
                settings={workflowSettings}
                tags={workflowTags}
                metadata={workflowMetadata}
                onSettingsChange={setWorkflowSettings}
                onTagsChange={setWorkflowTags}
                onMetadataChange={setWorkflowMetadata}
              />
            </div>
          )}

          {selectedNode && (
            <div className="p-4">
              <PropertiesPanel
                node={selectedNode}
                onClose={() => setSelectedNodes(new Set())}
                onUpdate={handleUpdateNode}
              />
            </div>
          )}

          {!selectedNode && !showSettings && (
            <div className="p-8 text-center text-gray-500">
              <p className="mb-2">Select a node to edit its properties</p>
              <p className="text-sm">or click the + button to add a new node</p>
            </div>
          )}
        </div>
      </div>

      {/* Node Library Modal */}
      {showNodeLibrary && (
        <NodeLibrary
          onAddNode={addNode}
          onClose={() => setShowNodeLibrary(false)}
        />
      )}
    </div>
  );
};

export default ImprovedCanvas;
