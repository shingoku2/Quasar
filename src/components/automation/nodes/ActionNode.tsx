import React from 'react';
import { NodeData } from '../Canvas';
import { Terminal, FileUp } from 'lucide-react';

interface ActionNodeProps {
  node: NodeData;
  isSelected: boolean;
  onMouseDown: (e: React.MouseEvent) => void;
  onPortMouseDown: (e: React.MouseEvent, port: string) => void;
  onPortMouseUp: (e: React.MouseEvent, port: string) => void;
}

const ActionNode: React.FC<ActionNodeProps> = ({
  node,
  isSelected,
  onMouseDown,
  onPortMouseDown,
  onPortMouseUp,
}) => {
  const actionType = node.data.config?.actionType || 'ssh';
  
  const getIcon = () => {
    switch (actionType) {
      case 'file_transfer':
        return <FileUp className="w-4 h-4" />;
      default:
        return <Terminal className="w-4 h-4" />;
    }
  };

  const getLabel = () => {
    switch (actionType) {
      case 'file_transfer':
        return 'Transfer File';
      default:
        return 'SSH Command';
    }
  };

  return (
    <g
      transform={`translate(${node.position.x}, ${node.position.y})`}
      onMouseDown={onMouseDown}
      className="cursor-move"
    >
      {/* Shadow */}
      <rect
        x={2}
        y={4}
        width={148}
        height={78}
        rx={8}
        fill="rgba(0,0,0,0.3)"
      />
      {/* Body */}
      <rect
        x={0}
        y={0}
        width={150}
        height={80}
        rx={8}
        fill="#1a1a1a"
        stroke={isSelected ? '#3b82f6' : '#333'}
        strokeWidth={isSelected ? 2 : 1}
      />
      {/* Header */}
      <rect
        x={0}
        y={0}
        width={150}
        height={28}
        rx={8}
        fill="#3b82f6"
      />
      <rect
        x={0}
        y={12}
        width={150}
        height={16}
        fill="#3b82f6"
      />
      {/* Icon */}
      <foreignObject x={8} y={4} width={20} height={20}>
        <div className="text-white/80">{getIcon()}</div>
      </foreignObject>
      {/* Title */}
      <text
        x={75}
        y={18}
        textAnchor="middle"
        fill="white"
        fontSize={11}
        fontWeight="bold"
        fontFamily="system-ui"
      >
        {getLabel()}
      </text>
      {/* Label */}
      <text
        x={75}
        y={55}
        textAnchor="middle"
        fill="#fff"
        fontSize={12}
        fontFamily="system-ui"
      >
        {node.data.label}
      </text>
      {/* Input port */}
      <circle
        cx={0}
        cy={40}
        r={6}
        fill="#333"
        stroke="#555"
        strokeWidth={2}
        className="cursor-crosshair hover:fill-accent"
        onMouseUp={(e) => onPortMouseUp(e, 'input')}
      />
      {/* Output port */}
      <circle
        cx={150}
        cy={40}
        r={6}
        fill="#333"
        stroke="#555"
        strokeWidth={2}
        className="cursor-crosshair hover:fill-accent"
        onMouseDown={(e) => onPortMouseDown(e, 'output')}
      />
    </g>
  );
};

export default ActionNode;
