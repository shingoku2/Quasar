import React from 'react';
import { Search, ChevronRight, Bell, User, Maximize2, Minus, X } from 'lucide-react';
import { ViewId } from './Sidebar';

interface TopBarProps {
  activeView: ViewId;
}

const TopBar: React.FC<TopBarProps> = ({ activeView }) => {
  const viewLabel = activeView.charAt(0).toUpperCase() + activeView.slice(1);

  return (
    <header className="h-14 border-b border-gray-800 bg-bg-root flex items-center justify-between px-4 sticky top-0 z-10">
      {/* Breadcrumbs */}
      <div className="flex items-center space-x-2 text-xs font-medium text-gray-500">
        <span className="hover:text-gray-300 cursor-pointer transition-colors">Titan</span>
        <ChevronRight className="h-3 w-3" />
        <span className="text-gray-200">{viewLabel}</span>
      </div>

      {/* Global Search */}
      <div className="flex-1 max-w-xl px-8">
        <div className="relative group">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500 group-focus-within:text-accent transition-colors" />
          <input 
            type="text" 
            placeholder="Search resources, commands, logs... (Ctrl+K)"
            className="w-full bg-zinc-850/50 border border-gray-800 rounded-full py-1.5 pl-10 pr-4 text-sm text-gray-300 focus:outline-none focus:border-accent/50 focus:bg-zinc-850 transition-all placeholder:text-gray-600"
          />
        </div>
      </div>

      {/* Actions & Controls */}
      <div className="flex items-center space-x-4">
        <div className="flex items-center space-x-2 pr-4 border-r border-gray-800">
          <button className="p-1.5 text-gray-500 hover:text-gray-200 hover:bg-zinc-800 rounded-lg transition-all relative">
            <Bell className="h-4 w-4" />
            <span className="absolute top-1.5 right-1.5 h-1.5 w-1.5 bg-red-500 rounded-full border border-bg-root" />
          </button>
          <button className="p-1.5 text-gray-500 hover:text-gray-200 hover:bg-zinc-800 rounded-lg transition-all text-xs font-mono font-bold">
            <User className="h-4 w-4" />
          </button>
        </div>
      </div>
    </header>
  );
};

export default TopBar;
