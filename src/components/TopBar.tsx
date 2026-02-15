import React from 'react';
import { Search } from 'lucide-react';
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
    <header className="h-14 border-b border-border bg-bg-root flex items-center px-6 sticky top-0 z-10">
      {/* View Title */}
      <h1 className="text-lg font-bold text-white">{viewLabels[activeView]}</h1>

      {/* Global Search */}
      <div className="flex-1 max-w-md pl-8 ml-auto">
        <div className="relative group">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500 group-focus-within:text-accent transition-colors" />
          <input 
            type="text" 
            placeholder="Search hosts, credentials..."
            className="w-full bg-bg-card border border-border rounded-full py-1.5 pl-10 pr-4 text-sm text-gray-300 focus:outline-none focus:border-accent/50 focus:bg-bg-sidebar transition-all placeholder:text-gray-500"
          />
        </div>
      </div>
    </header>
  );
};

export default TopBar;
