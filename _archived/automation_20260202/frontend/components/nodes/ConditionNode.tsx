import React from 'react';
import { NodeData } from '../Canvas';
import { GitBranch } from 'lucide-react';

interface ConditionNodeProps {
  node: NodeData;
  isSelected: boolean;
  onMouseDown: (e: React.MouseEvent) => void;
  onPortMouseDown: (e: React.MouseEvent, port: string) => void;
  onPortMouseUp: (e: React.MouseEvent, port: string) => void;
}

const ConditionNode: React.FC<ConditionNodeProps> = ({
  node,
  isSelected,
  onMouseDown,
  onPortMouseDown,
  onPortMouseUp,
}) => {
  const condition = node.data.config?.expression || 'true';

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
        height={98}
        rx={8}
        fill="rgba(0,0,0,0.3)"
      />
      {/* Body */}
      <rect
        x={0}
        y={0}
        width={150}
        height={100}
        rx={8}
        fill="#1a1a1a"
        stroke={isSelected ? '#f59e0b' : '#333'}
        strokeWidth={isSelected ? 2 : 1}
      />
      {/* Header */}
      <rect
        x={0}
        y={0}
        width={150}
        height={28}
        rx={8}
        fill="#f59e0b"
      />
      <rect
        x={0}
        y={12}
        width={150}
        height={16}
        fill="#f59e0b"
      />
      {/* Icon */}
      <foreignObject x={8} y={4} width={20} height={20}>
        <div className="text-white/80">
          <GitBranch className="w-4 h-4" />
        </div>
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
        Condition
      </text>
      {/* Expression preview */}
      <text
        x={75}
        y={50}
        textAnchor="middle"
        fill="#aaa"
        fontSize={10}
        fontFamily="monospace"
      >
        {condition.length > 20 ? condition.substring(0, 20) + '...' : condition}
      </text>
      {/* Label */}
      <text
        x={75}
        y={70}
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
        cy={50}
        r={6}
        fill="#333"
        stroke="#555"
        strokeWidth={2}
        className="cursor-crosshair hover:fill-accent"
        onMouseUp={(e) => onPortMouseUp(e, 'input')}
      />
      {/* True output port (top) */}
      <circle
        cx={75}
        cy={0}
        r={6}
        fill="#10b981"
        stroke="#555"
        strokeWidth={2}
        className="cursor-crosshair hover:fill-accent"
        onMouseDown={(e) => onPortMouseDown(e, 'true')}
      />
      <text x={75} y={-10} textAnchor="middle" fill="#10b981" fontSize={10}>True</text>
      {/* False output port (right) */}
      <circle
        cx={150}
        cy={50}
        r={6}
        fill="#ef4444"
        stroke="#555"
        strokeWidth={2}
        className="cursor-crosshair hover:fill-accent"
        onMouseDown={(e) => onPortMouseDown(e, 'false')}
      />
      <text x={165} y={54} fill="#ef4444" fontSize={10}>False</text>
    </g>
  );
};

export default ConditionNode;
