import React from 'react';
import { 
  LayoutDashboard, 
  Terminal, 
  Activity, 
  Workflow, 
  Bot, 
  Settings,
  Shield
} from 'lucide-react';
import { cn } from '../lib/utils';

export type ViewId = 'dashboard' | 'remote' | 'monitoring' | 'automation' | 'ai' | 'settings';

interface SidebarProps {
  activeView: ViewId;
  onViewChange: (view: ViewId) => void;
}

const navItems = [
  { id: 'dashboard', icon: LayoutDashboard, label: 'Dashboard' },
  { id: 'remote', icon: Terminal, label: 'Remote' },
  { id: 'monitoring', icon: Activity, label: 'Monitoring' },
  { id: 'automation', icon: Workflow, label: 'Automation' },
  { id: 'ai', icon: Bot, label: 'AI Assistant' },
  { id: 'settings', icon: Settings, label: 'Settings' },
] as const;

const Sidebar: React.FC<SidebarProps> = ({ activeView, onViewChange }) => {
  return (
    <aside className="w-16 lg:w-64 bg-bg-sidebar border-r border-gray-800 flex flex-col h-full transition-all duration-300">
      <div className="p-4 lg:p-6 flex items-center space-x-3 border-b border-gray-800 mb-2">
        <div className="bg-accent p-1.5 rounded-lg shadow-lg shadow-accent/20">
          <Shield className="h-6 w-6 text-white" />
        </div>
        <span className="font-bold text-lg tracking-tight hidden lg:block text-white">TITAN NEXUS</span>
      </div>

      <nav className="flex-1 px-2 space-y-1 py-4 overflow-y-auto no-scrollbar">
        {navItems.map((item) => {
          const isActive = activeView === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onViewChange(item.id as ViewId)}
              className={cn(
                "w-full flex items-center rounded-lg px-3 py-2.5 transition-all group relative",
                isActive 
                  ? "bg-accent/10 text-accent font-medium shadow-sm" 
                  : "text-gray-400 hover:bg-gray-800 hover:text-gray-200"
              )}
              title={item.label}
            >
              <item.icon className={cn(
                "h-5 w-5 shrink-0",
                isActive ? "text-accent" : "text-gray-400 group-hover:text-gray-200"
              )} />
              <span className="ml-3 text-sm hidden lg:block truncate">{item.label}</span>
              
              {isActive && (
                <div className="absolute left-0 top-2 bottom-2 w-1 bg-accent rounded-r-full lg:hidden" />
              )}
            </button>
          );
        })}
      </nav>

      <div className="p-4 border-t border-gray-800 hidden lg:block">
        <div className="flex items-center space-x-3 px-2 py-2">
          <div className="h-8 w-8 rounded-full bg-zinc-800 flex items-center justify-center border border-gray-700">
            <span className="text-xs font-bold text-accent">Sys</span>
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-xs font-bold text-white truncate leading-none">Admin Nexus</p>
            <p className="text-[10px] text-gray-500 mt-1 truncate">v0.1.0-alpha</p>
          </div>
        </div>
      </div>
    </aside>
  );
};

export default Sidebar;
