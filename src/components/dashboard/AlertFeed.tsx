import React from 'react';
import { AlertCircle, Clock } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface Alert {
  id: string;
  source: string;
  message: string;
  severity: 'critical' | 'warning' | 'info';
  timestamp: string;
}

interface AlertFeedProps {
  alerts: Alert[];
}

const AlertFeed: React.FC<AlertFeedProps> = ({ alerts }) => {
  return (
    <div className="bg-bg-card border border-gray-800 rounded-xl flex flex-col h-full overflow-hidden shadow-sm">
      <div className="p-4 border-b border-gray-800 flex justify-between items-center bg-bg-card/50">
        <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest">Recent Alerts</h3>
        <span className="text-[10px] bg-zinc-800 text-gray-500 px-2 py-0.5 rounded-full font-mono">LIVE</span>
      </div>
      
      <div className="flex-1 overflow-y-auto no-scrollbar">
        {alerts.length === 0 ? (
          <div className="p-8 text-center text-gray-600 italic text-sm">No active alerts.</div>
        ) : (
          <div className="divide-y divide-gray-800/50">
            {alerts.map((alert) => (
              <div 
                key={alert.id} 
                className={cn(
                  "p-3 flex items-start space-x-3 transition-colors hover:bg-white/5 border-l-4",
                  alert.severity === 'critical' ? "border-alert" : 
                  alert.severity === 'warning' ? "border-warning" : "border-accent"
                )}
              >
                <AlertCircle className={cn(
                  "h-4 w-4 mt-0.5 shrink-0",
                  alert.severity === 'critical' ? "text-alert" : 
                  alert.severity === 'warning' ? "text-warning" : "text-accent"
                )} />
                <div className="flex-1 min-w-0">
                  <div className="flex justify-between items-start">
                    <p className="text-xs font-bold text-gray-300 truncate">{alert.source}</p>
                    <div className="flex items-center text-[10px] text-gray-500 font-mono shrink-0 ml-2">
                      <Clock className="h-3 w-3 mr-1" />
                      {alert.timestamp}
                    </div>
                  </div>
                  <p className="text-xs text-gray-400 mt-1 line-clamp-2 leading-relaxed">{alert.message}</p>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
      
      <div className="p-2 border-t border-gray-800 bg-bg-card/30">
        <button className="w-full py-1.5 text-[10px] font-bold text-gray-500 hover:text-gray-300 uppercase tracking-widest transition-colors">
          View Audit Log
        </button>
      </div>
    </div>
  );
};

export default AlertFeed;
