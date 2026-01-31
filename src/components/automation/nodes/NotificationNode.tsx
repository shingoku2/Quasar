import React from 'react';
import { NodeData } from '../Canvas';
import { Bell, MessageSquare, Monitor } from 'lucide-react';

interface NotificationNodeProps {
  node: NodeData;
  isSelected: boolean;
  onMouseDown: (e: React.MouseEvent) => void;
  onPortMouseDown: (e: React.MouseEvent, port: string) => void;
  onPortMouseUp: (e: React.MouseEvent, port: string) => void;
}

const NotificationNode: React.FC<NotificationNodeProps> = ({
  node,
  isSelected,
  onMouseDown,
  onPortMouseDown,
  onPortMouseUp,
}) => {
  const channel = node.data.config?.channel || 'in_app';
  
  const getIcon = () => {
    switch (channel) {
      case 'system':
        return <Monitor className="w-4 h-4" />;
      case 'both':
        return <MessageSquare className="w-4 h-4" />;
      default:
        return <Bell className="w-4 h-4" />;
    }
  };

  const getLabel = () => {
    switch (channel) {
      case 'system':
        return 'System Notification';
      case 'both':
        return 'All Channels';
      default:
        return 'In-App Notification';
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
        stroke={isSelected ? '#8b5cf6' : '#333'}
        strokeWidth={isSelected ? 2 : 1}
      />
      {/* Header */}
      <rect
        x={0}
        y={0}
        width={150}
        height={28}
        rx={8}
        fill="#8b5cf6"
      />
      <rect
        x={0}
        y={12}
        width={150}
        height={16}
        fill="#8b5cf6"
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
      {/* Output port (notifications can chain) */}
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

export default NotificationNode;
