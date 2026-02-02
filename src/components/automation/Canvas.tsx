import React, { useState, useRef, useCallback, useEffect } from 'react';
import { ZoomIn, ZoomOut, MousePointer2, Trash2, Play, Save, Download, Upload } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import TriggerNode from './nodes/TriggerNode';
import ActionNode from './nodes/ActionNode';
import ConditionNode from './nodes/ConditionNode';
import NotificationNode from './nodes/NotificationNode';
import PropertiesPanel from './PropertiesPanel';

export interface NodeData {
  id: string;
  type: 'trigger' | 'action' | 'condition' | 'notification';
  position: { x: number; y: number };
  data: {
    label: string;
    config?: Record<string, any>;
  };
}

export interface ConnectionData {
  id: string;
  source: string;
  target: string;
  sourcePort?: string;
  targetPort?: string;
}

interface CanvasProps {
  workflowId?: string;
  initialNodes?: NodeData[];
  initialConnections?: ConnectionData[];
  onSave?: (nodes: NodeData[], connections: ConnectionData[]) => void;
}

const GRID_SIZE = 20;
const MIN_ZOOM = 0.25;
const MAX_ZOOM = 2;

const Canvas: React.FC<CanvasProps> = ({
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
  
  const canvasRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ nodeId: string; offsetX: number; offsetY: number } | null>(null);

  // Grid background pattern
  const gridPattern = `url("data:image/svg+xml,%3Csvg width='${GRID_SIZE}' height='${GRID_SIZE}' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M ${GRID_SIZE} 0 L 0 0 0 ${GRID_SIZE}' fill='none' stroke='%23333' stroke-width='0.5'/%3E%3C/svg%3E")`;

  const snapToGrid = (value: number) => Math.round(value / GRID_SIZE) * GRID_SIZE;

  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      const delta = e.deltaY > 0 ? 0.9 : 1.1;
      setZoom(z => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z * delta)));
    } else {
      setPan(p => ({ x: p.x - e.deltaX, y: p.y - e.deltaY }));
    }
  }, []);

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button === 1 || (e.button === 0 && e.shiftKey)) {
      setIsPanning(true);
      e.preventDefault();
    } else if (e.button === 0 && e.target === canvasRef.current) {
      setSelectedNodes(new Set());
    }
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (isPanning) {
      setPan(p => ({ x: p.x + e.movementX, y: p.y + e.movementY }));
      return;
    }

    if (dragRef.current) {
      const { nodeId, offsetX, offsetY } = dragRef.current;
      const rect = canvasRef.current?.getBoundingClientRect();
      if (rect) {
        const x = (e.clientX - rect.left - pan.x - offsetX) / zoom;
        const y = (e.clientY - rect.top - pan.y - offsetY) / zoom;
        
        setNodes(prev => prev.map(node =>
          node.id === nodeId
            ? { ...node, position: { x: snapToGrid(x), y: snapToGrid(y) } }
            : node
        ));
      }
    }

    if (isConnecting && tempConnection) {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (rect) {
        setTempConnection({
          x: (e.clientX - rect.left - pan.x) / zoom,
          y: (e.clientY - rect.top - pan.y) / zoom,
        });
      }
    }
  };

  const handleMouseUp = () => {
    dragRef.current = null;
    setIsPanning(false);
  };

  const handleNodeMouseDown = (e: React.MouseEvent, nodeId: string) => {
    if (e.button !== 0) return;
    
    const rect = canvasRef.current?.getBoundingClientRect();
    const node = nodes.find(n => n.id === nodeId);
    if (!rect || !node) return;

    const mouseX = (e.clientX - rect.left - pan.x) / zoom;
    const mouseY = (e.clientY - rect.top - pan.y) / zoom;
    
    dragRef.current = {
      nodeId,
      offsetX: mouseX - node.position.x,
      offsetY: mouseY - node.position.y,
    };

    if (!e.ctrlKey && !e.metaKey) {
      setSelectedNodes(new Set([nodeId]));
    } else {
      setSelectedNodes(prev => {
        const next = new Set(prev);
        if (next.has(nodeId)) next.delete(nodeId);
        else next.add(nodeId);
        return next;
      });
    }
    
    e.stopPropagation();
  };

  const handleUpdateNode = (nodeId: string, data: Partial<NodeData['data']>) => {
    setNodes(prev => prev.map(n => 
      n.id === nodeId ? { ...n, data: { ...n.data, ...data } } : n
    ));
  };

  const selectedNode = selectedNodes.size === 1 
    ? nodes.find(n => n.id === Array.from(selectedNodes)[0]) || null 
    : null;

  const handlePortMouseDown = (e: React.MouseEvent, nodeId: string, port: string) => {
    e.stopPropagation();
    setIsConnecting(true);
    setConnectingFrom({ nodeId, port });
    setTempConnection({ x: 0, y: 0 });
  };

  const handlePortMouseUp = (e: React.MouseEvent, nodeId: string, port: string) => {
    if (isConnecting && connectingFrom && connectingFrom.nodeId !== nodeId) {
      const newConnection: ConnectionData = {
        id: `conn-${Date.now()}`,
        source: connectingFrom.nodeId,
        target: nodeId,
        sourcePort: connectingFrom.port,
        targetPort: port,
      };
      setConnections(prev => [...prev, newConnection]);
    }
    setIsConnecting(false);
    setConnectingFrom(null);
    setTempConnection(null);
    e.stopPropagation();
  };

  const deleteSelected = () => {
    setNodes(prev => prev.filter(n => !selectedNodes.has(n.id)));
    setConnections(prev => prev.filter(c => !selectedNodes.has(c.source) && !selectedNodes.has(c.target)));
    setSelectedNodes(new Set());
  };

  const addNode = (type: NodeData['type'], position: { x: number; y: number }) => {
    const newNode: NodeData = {
      id: `node-${Date.now()}`,
      type,
      position: { x: snapToGrid(position.x), y: snapToGrid(position.y) },
      data: { label: `${type.charAt(0).toUpperCase() + type.slice(1)} Node` },
    };
    setNodes(prev => [...prev, newNode]);
  };

  const executeWorkflow = async () => {
    const wfId = workflowId || `workflow-${Date.now()}`;
    
    // Convert canvas nodes to workflow format matching backend NodeConfig
    const workflow = {
      id: wfId,
      name: 'Untitled Workflow',
      nodes: nodes.map(n => {
        let config: any;
        
        // Check if node has old-format config and convert it
        const existingConfig = n.data.config;
        if (!existingConfig) {
          // Build default config based on node type
          switch (n.type) {
            case 'trigger':
              config = { 
                type: 'trigger',
                trigger_type: 'manual' 
              };
              break;
            case 'action':
              config = { 
                type: 'action',
                action_type: {
                  ssh_command: {
                    host: 'localhost',
                    port: 22,
                    username: 'admin',
                    password: null,
                    command: 'echo "Hello from automation"'
                  }
                }
              };
              break;
            case 'condition':
              config = { 
                type: 'condition',
                expression: 'true',
                true_branch: '',
                false_branch: ''
              };
              break;
            case 'notification':
              config = {
                type: 'notification',
                title: 'Workflow Notification',
                message: 'Workflow executed successfully',
                channel: 'in_app'
              };
              break;
            default:
              config = {};
          }
        } else {
          // Use existing config or build new one
          switch (n.type) {
            case 'trigger':
              // Ensure trigger_type is in correct format (string for manual, not object)
              if (existingConfig?.trigger_type) {
                config = { ...existingConfig };
                // Fix nested type format if present
                if (typeof config.trigger_type === 'object' && config.trigger_type.type === 'manual') {
                  config.trigger_type = 'manual';
                }
              } else {
                config = { 
                  type: 'trigger',
                  trigger_type: 'manual' 
                };
              }
              break;
            case 'action':
              // Ensure action_type is in correct format
              if (existingConfig?.action_type) {
                config = { ...existingConfig };
                // Fix nested type format if present
                if (config.action_type.type === 'ssh_command') {
                  const { type, ...rest } = config.action_type;
                  config.action_type = { ssh_command: rest };
                }
              } else {
                config = { 
                  type: 'action',
                  action_type: {
                    ssh_command: {
                      host: 'localhost',
                      port: 22,
                      username: 'admin',
                      password: null,
                      command: 'echo "Hello from automation"'
                    }
                  }
                };
              }
              break;
            case 'condition':
              config = existingConfig || { 
                type: 'condition',
                expression: 'true',
                true_branch: '',
                false_branch: ''
              };
              break;
            case 'notification':
              config = existingConfig || {
                type: 'notification',
                title: 'Workflow Notification',
                message: 'Workflow executed successfully',
                channel: 'in_app'
              };
              break;
            default:
              config = existingConfig || {};
          }
        }
        
        return {
          id: n.id,
          node_type: n.type,
          name: n.data.label,
          position: n.position,
          config: config
        };
      }),
      connections: connections.map(c => ({
        id: c.id,
        source_node: c.source,
        target_node: c.target,
        source_port: c.sourcePort || 'output',
        target_port: c.targetPort || 'input'
      })),
      enabled: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString()
    };

    try {
      // Debug: log the workflow structure
      console.log('Saving workflow:', JSON.stringify(workflow, null, 2));
      
      // First save the workflow
      await invoke('save_workflow', { workflow });
      console.log('Workflow saved:', wfId);
      
      // Then execute it
      const execId = await invoke<string>('execute_workflow', { workflowId: wfId });
      console.log('Workflow execution started:', execId);
    } catch (err) {
      console.error('Failed to execute workflow:', err);
    }
  };

  // Load workflow from backend on mount
  useEffect(() => {
    const loadWorkflow = async () => {
      if (!workflowId) return;
      
      try {
        const workflow = await invoke<any>('get_workflow', { id: workflowId });
        if (workflow && workflow.nodes) {
          // Convert backend nodes to canvas format
          const canvasNodes: NodeData[] = workflow.nodes.map((node: any) => ({
            id: node.id,
            type: node.node_type,
            position: node.position,
            data: {
              label: node.name,
              config: node.config,
            },
          }));
          
          const canvasConnections: ConnectionData[] = workflow.connections.map((conn: any) => ({
            id: conn.id,
            source: conn.source_node,
            target: conn.target_node,
            sourcePort: conn.source_port,
            targetPort: conn.target_port,
          }));
          
          setNodes(canvasNodes);
          setConnections(canvasConnections);
        }
      } catch (err) {
        console.error('Failed to load workflow:', err);
      }
    };
    
    loadWorkflow();
  }, [workflowId]);

  // Auto-save workflow when component unmounts (switching views)
  useEffect(() => {
    return () => {
      if (!workflowId || nodes.length === 0) return;
      
      // Save workflow on unmount
      const saveOnUnmount = async () => {
        try {
          const workflow = {
            id: workflowId,
            name: 'Untitled Workflow',
            nodes: nodes.map(n => {
              // Use existing config if available, ensuring proper format
              let config = n.data.config;
              
              // If no config, provide defaults
              if (!config) {
                config = getDefaultConfig(n.type);
              } else {
                // Ensure config is in correct format (same logic as executeWorkflow)
                if (n.type === 'trigger' && config.trigger_type) {
                  // Ensure trigger_type is a string, not an object
                  if (typeof config.trigger_type === 'object' && config.trigger_type.type) {
                    config = { ...config, trigger_type: config.trigger_type.type };
                  }
                } else if (n.type === 'action' && config.action_type) {
                  // Ensure action_type is properly nested
                  if (config.action_type.type === 'ssh_command') {
                    const { type, ...rest } = config.action_type;
                    config = { ...config, action_type: { ssh_command: rest } };
                  }
                }
              }
              
              return {
                id: n.id,
                node_type: n.type,
                name: n.data.label,
                position: n.position,
                config: config,
              };
            }),
            connections: connections.map(c => ({
              id: c.id,
              source_node: c.source,
              target_node: c.target,
              source_port: c.sourcePort || 'output',
              target_port: c.targetPort || 'input',
            })),
            enabled: true,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          
          await invoke('save_workflow', { workflow });
        } catch (err) {
          console.error('Failed to auto-save workflow:', err);
        }
      };
      
      saveOnUnmount();
    };
  }, [workflowId, nodes, connections]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.key === 'Delete' || e.key === 'Backspace') && selectedNodes.size > 0) {
        deleteSelected();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedNodes, deleteSelected]);
  
  // Helper function to get default config for node types
  const getDefaultConfig = (type: string) => {
    switch (type) {
      case 'trigger':
        return { type: 'trigger', trigger_type: 'manual' };
      case 'action':
        return {
          type: 'action',
          action_type: {
            ssh_command: {
              host: 'localhost',
              port: 22,
              username: 'admin',
              password: null,
              command: 'echo "Hello"',
            },
          },
        };
      case 'condition':
        return { type: 'condition', expression: 'true', true_branch: '', false_branch: '' };
      case 'notification':
        return { type: 'notification', title: 'Notification', message: 'Message', channel: 'in_app' };
      default:
        return {};
    }
  };

  const nodeColors = {
    trigger: '#10b981',
    action: '#3b82f6',
    condition: '#f59e0b',
    notification: '#8b5cf6',
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

    return (
      <g key={conn.id}>
        <path
          d={path}
          fill="none"
          stroke="#555"
          strokeWidth={2}
          className="hover:stroke-accent cursor-pointer"
          onClick={() => setConnections(prev => prev.filter(c => c.id !== conn.id))}
        />
      </g>
    );
  };

  return (
    <div ref={containerRef} className="flex h-full bg-zinc-950">
      {/* Node Palette */}
      <div className="w-16 border-r border-gray-800 bg-bg-card flex flex-col items-center py-4 space-y-4">
        <div className="text-[10px] text-gray-500 uppercase tracking-wider">Nodes</div>
        {(['trigger', 'action', 'condition', 'notification'] as const).map(type => (
          <button
            key={type}
            onClick={() => addNode(type, { x: 100 + Math.random() * 200, y: 100 + Math.random() * 200 })}
            className="w-10 h-10 rounded-lg flex items-center justify-center transition-all hover:scale-110"
            style={{ backgroundColor: nodeColors[type] + '20', border: `1px solid ${nodeColors[type]}` }}
            title={`Add ${type} node`}
          >
            <div className="w-3 h-3 rounded-full" style={{ backgroundColor: nodeColors[type] }} />
          </button>
        ))}
      </div>

      {/* Canvas */}
      <div className="flex-1 relative overflow-hidden">
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
            {/* Connections */}
            {connections.map(renderConnection)}
            
            {/* Temp connection while dragging */}
            {isConnecting && connectingFrom && tempConnection && (() => {
              const sourceNode = nodes.find(n => n.id === connectingFrom.nodeId);
              if (!sourceNode) return null;
              const x1 = sourceNode.position.x + 150;
              const y1 = sourceNode.position.y + 40;
              const path = `M ${x1} ${y1} L ${tempConnection.x} ${tempConnection.y}`;
              return (
                <path
                  d={path}
                  fill="none"
                  stroke="#10b981"
                  strokeWidth={2}
                  strokeDasharray="5,5"
                />
              );
            })()}
            
            {/* Nodes */}
            {nodes.map(node => {
              const isSelected = selectedNodes.has(node.id);
              const commonProps = {
                node,
                isSelected,
                onMouseDown: (e: React.MouseEvent) => handleNodeMouseDown(e, node.id),
                onPortMouseDown: (e: React.MouseEvent, port: string) => handlePortMouseDown(e, node.id, port),
                onPortMouseUp: (e: React.MouseEvent, port: string) => handlePortMouseUp(e, node.id, port),
              };
              
              switch (node.type) {
                case 'trigger':
                  return <TriggerNode key={node.id} {...commonProps} />;
                case 'action':
                  return <ActionNode key={node.id} {...commonProps} />;
                case 'condition':
                  return <ConditionNode key={node.id} {...commonProps} />;
                case 'notification':
                  return <NotificationNode key={node.id} {...commonProps} />;
                default:
                  return null;
              }
            })}
          </g>
        </svg>

        {/* Toolbar */}
        <div className="absolute top-4 left-4 right-4 flex justify-between items-center">
          <div className="flex items-center space-x-2 bg-bg-card/90 backdrop-blur border border-gray-800 rounded-lg px-3 py-2">
            <button
              onClick={() => setZoom(z => Math.max(MIN_ZOOM, z - 0.1))}
              className="p-1.5 hover:bg-white/10 rounded text-gray-400 hover:text-white"
            >
              <ZoomOut className="w-4 h-4" />
            </button>
            <span className="text-xs text-gray-400 w-12 text-center">{Math.round(zoom * 100)}%</span>
            <button
              onClick={() => setZoom(z => Math.min(MAX_ZOOM, z + 0.1))}
              className="p-1.5 hover:bg-white/10 rounded text-gray-400 hover:text-white"
            >
              <ZoomIn className="w-4 h-4" />
            </button>
            <div className="w-px h-4 bg-gray-700 mx-2" />
            <button
              onClick={() => { setZoom(1); setPan({ x: 0, y: 0 }); }}
              className="p-1.5 hover:bg-white/10 rounded text-gray-400 hover:text-white"
              title="Reset view"
            >
              <MousePointer2 className="w-4 h-4" />
            </button>
          </div>

          <div className="flex items-center space-x-2 bg-bg-card/90 backdrop-blur border border-gray-800 rounded-lg px-3 py-2">
            {selectedNodes.size > 0 && (
              <button
                onClick={deleteSelected}
                className="p-1.5 hover:bg-red-500/20 rounded text-red-400 hover:text-red-300"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            )}
            <button
              onClick={() => onSave?.(nodes, connections)}
              className="p-1.5 hover:bg-white/10 rounded text-gray-400 hover:text-white"
            >
              <Save className="w-4 h-4" />
            </button>
            <button
              onClick={executeWorkflow}
              className="flex items-center space-x-1 px-3 py-1.5 bg-accent/20 hover:bg-accent/30 text-accent rounded text-xs font-bold uppercase tracking-wider"
            >
              <Play className="w-3 h-3" />
              <span>Run</span>
            </button>
          </div>
        </div>

        {/* Status bar */}
        <div className="absolute bottom-4 left-4 right-4 flex justify-between items-center">
          <div className="text-xs text-gray-500">
            {nodes.length} nodes · {connections.length} connections
            {selectedNodes.size > 0 && ` · ${selectedNodes.size} selected`}
          </div>
          <div className="flex items-center space-x-2">
            <button
              onClick={() => {
                // Build proper configs for export
                const nodesWithConfigs = nodes.map(n => {
                  let config: any;
                  const existingConfig = n.data.config;
                  
                  // Use existing config if available, otherwise create defaults
                  if (existingConfig && Object.keys(existingConfig).length > 0) {
                    config = existingConfig;
                  } else {
                    // Create default configs with type field
                    switch (n.type) {
                      case 'trigger':
                        config = {
                          type: 'trigger',
                          trigger_type: 'manual'
                        };
                        break;
                      case 'action':
                        config = {
                          type: 'action',
                          action_type: {
                            ssh_command: {
                              host: 'localhost',
                              port: 22,
                              username: 'admin',
                              password: null,
                              command: 'echo "Hello from automation"'
                            }
                          }
                        };
                        break;
                      case 'condition':
                        config = {
                          type: 'condition',
                          expression: 'true',
                          true_branch: '',
                          false_branch: ''
                        };
                        break;
                      case 'notification':
                        config = {
                          type: 'notification',
                          title: 'Workflow Notification',
                          message: 'Workflow executed successfully',
                          channel: 'in_app'
                        };
                        break;
                      default:
                        config = {};
                    }
                  }
                  
                  // Ensure config has type field
                  if (!config.type) {
                    config = { type: n.type, ...config };
                  }
                  
                  return {
                    id: n.id,
                    node_type: n.type,
                    name: n.data.label,
                    position: n.position,
                    config: config
                  };
                });
                
                const workflow = {
                  id: workflowId || `workflow-${Date.now()}`,
                  name: 'Untitled Workflow',
                  nodes: nodesWithConfigs,
                  connections,
                  created_at: new Date().toISOString(),
                  updated_at: new Date().toISOString()
                };
                const blob = new Blob([JSON.stringify(workflow, null, 2)], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = `${workflow.id}.json`;
                a.click();
                URL.revokeObjectURL(url);
              }}
              className="p-1.5 hover:bg-white/10 rounded text-gray-400 hover:text-white"
              title="Export workflow"
            >
              <Download className="w-4 h-4" />
            </button>
            <button
              onClick={() => {
                const input = document.createElement('input');
                input.type = 'file';
                input.accept = '.json';
                input.onchange = async (e) => {
                  const file = (e.target as HTMLInputElement).files?.[0];
                  if (!file) return;
                  try {
                    const text = await file.text();
                    const workflow = JSON.parse(text);
                    if (workflow.nodes) {
                      setNodes(workflow.nodes.map((n: any) => {
                        // Get config and node type
                        let config = n.config || n.data?.config;
                        const nodeType = n.node_type || n.type;
                        
                        console.log('Importing node:', nodeType, 'with config:', config);
                        
                        // Ensure config has the type field required by backend
                        if (!config?.type) {
                          // Config is missing type field, need to add it
                          if (config && Object.keys(config).length > 0) {
                            // Config exists but no type field - add type without spreading
                            // This prevents accidentally overwriting fields
                            const newConfig: any = { type: nodeType };
                            for (const key in config) {
                              if (key !== 'type') {
                                newConfig[key] = config[key];
                              }
                            }
                            config = newConfig;
                            console.log('Added type to existing config:', config);
                          } else {
                            // No config at all, create default with type
                            switch (nodeType) {
                              case 'trigger':
                                config = { type: 'trigger', trigger_type: 'manual' };
                                break;
                              case 'action':
                                config = { 
                                  type: 'action',
                                  action_type: {
                                    ssh_command: {
                                      host: 'localhost',
                                      port: 22,
                                      username: 'admin',
                                      password: null,
                                      command: 'echo "Hello from automation"'
                                    }
                                  }
                                };
                                break;
                              case 'condition':
                                config = { 
                                  type: 'condition',
                                  expression: 'true',
                                  true_branch: '',
                                  false_branch: ''
                                };
                                break;
                              case 'notification':
                                config = {
                                  type: 'notification',
                                  title: 'Workflow Notification',
                                  message: 'Workflow executed successfully',
                                  channel: 'in_app'
                                };
                                break;
                              default:
                                config = {};
                            }
                          }
                        }
                        
                        return {
                          id: n.id,
                          type: nodeType,
                          position: n.position,
                          data: {
                            label: n.name || n.data?.label || `${nodeType} Node`,
                            config: config
                          }
                        };
                      }));
                    }
                    if (workflow.connections) {
                      setConnections(workflow.connections);
                    }
                    console.log('Workflow imported:', workflow.id);
                  } catch (err) {
                    console.error('Failed to import workflow:', err);
                    alert('Failed to import workflow. Invalid file format.');
                  }
                };
                input.click();
              }}
              className="p-1.5 hover:bg-white/10 rounded text-gray-400 hover:text-white"
              title="Import workflow"
            >
              <Upload className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Properties Panel */}
      <PropertiesPanel
        node={selectedNode}
        onClose={() => setSelectedNodes(new Set())}
        onUpdate={handleUpdateNode}
      />
    </div>
  );
};

export default Canvas;
