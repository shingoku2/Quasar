import React from 'react';
import { ShieldCheck, Server, AlertTriangle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface SystemHealthWidgetProps {
  healthScore: number;
  statusText: string;
  nodes: { id: string; status: 'online' | 'offline' | 'warning' }[];
}

const SystemHealthWidget: React.FC<SystemHealthWidgetProps> = ({ healthScore, statusText, nodes }) => {
  return (
    <div className="bg-bg-card border border-gray-800 rounded-xl p-4 flex items-center justify-between shadow-sm">
      <div className="flex items-center space-x-4">
        <div className={cn(
          "h-12 w-12 rounded-full flex items-center justify-center border-4 shadow-inner transition-all duration-1000",
          healthScore > 90 ? "border-green-500/20 text-green-500" : 
          healthScore > 70 ? "border-warning/20 text-warning" : "border-alert/20 text-alert"
        )}>
          <span className="text-lg font-black">{healthScore}%</span>
        </div>
        <div>
          <h3 className="text-sm font-bold text-gray-400 uppercase tracking-wider leading-none">System Health</h3>
          <p className={cn(
            "text-xl font-black mt-1 tracking-tight",
            healthScore > 90 ? "text-green-500" : 
            healthScore > 70 ? "text-warning" : "text-alert"
          )}>{statusText}</p>
        </div>
      </div>

      <div className="hidden md:flex items-center space-x-6">
        <div className="flex items-center -space-x-2">
          {nodes.map((node) => (
            <div 
              key={node.id} 
              title={`Node ${node.id}: ${node.status}`}
              className={cn(
                "h-8 w-8 rounded-lg border-2 border-bg-card flex items-center justify-center transition-transform hover:-translate-y-1 cursor-help",
                node.status === 'online' ? "bg-green-500/10 text-green-500" : 
                node.status === 'warning' ? "bg-warning/10 text-warning" : "bg-alert/10 text-alert"
              )}
            >
              <Server className="h-4 w-4" />
            </div>
          ))}
        </div>
        <div className="text-right border-l border-gray-800 pl-6">
          <p className="text-[10px] font-bold text-gray-500 uppercase tracking-widest">Active Alerts</p>
          <div className="flex items-center mt-1 text-alert">
            <AlertTriangle className="h-4 w-4 mr-1.5" />
            <span className="text-lg font-black leading-none">3</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SystemHealthWidget;
