import React from 'react';
import { Search, Bell, User } from 'lucide-react';
import { ViewId } from './Sidebar';

interface TopBarProps {
  activeView: ViewId;
}

const viewLabels: Record<ViewId, string> = {
  dashboard: 'Dashboard',
  remote: 'Remote',
  monitoring: 'Monitoring',
  ai: 'AI Assistant',
  security: 'Security',
  settings: 'Settings',
};

const TopBar: React.FC<TopBarProps> = ({ activeView }) => {
  return (
    <header className="h-14 border-b border-border bg-bg-root flex items-center justify-between px-6 sticky top-0 z-10">
      {/* View Title */}
      <h1 className="text-lg font-bold text-white">{viewLabels[activeView]}</h1>

      {/* Global Search */}
      <div className="flex-1 max-w-md px-8">
        <div className="relative group">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500 group-focus-within:text-accent transition-colors" />
          <input 
            type="text" 
            placeholder="Search hosts, credentials..."
            className="w-full bg-bg-card border border-border rounded-full py-1.5 pl-10 pr-4 text-sm text-gray-300 focus:outline-none focus:border-accent/50 focus:bg-bg-sidebar transition-all placeholder:text-gray-500"
          />
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center space-x-3">
        <button className="p-2 text-gray-400 hover:text-white hover:bg-white/5 rounded-lg transition-all relative">
          <Bell className="h-4 w-4" />
          <span className="absolute top-1.5 right-1.5 h-1.5 w-1.5 bg-alert rounded-full border border-bg-root" />
        </button>
        <div className="h-8 w-8 rounded-full bg-bg-card border border-border flex items-center justify-center cursor-pointer hover:border-accent/50 transition-all">
          <User className="h-4 w-4 text-gray-400" />
        </div>
      </div>
    </header>
  );
};

export default TopBar;
