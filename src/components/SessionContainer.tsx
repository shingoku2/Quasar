import React from 'react';
import { X } from 'lucide-react';

export interface SessionTab {
  id: string;
  title: string;
  content: React.ReactNode;
  closable?: boolean; // Default to true
}

interface SessionContainerProps {
  tabs: SessionTab[];
  activeTabId: string;
  onTabChange: (id: string) => void;
  onTabClose: (id: string) => void;
  className?: string;
}

const SessionContainer: React.FC<SessionContainerProps> = ({
  tabs,
  activeTabId,
  onTabChange,
  onTabClose,
  className
}) => {
  return (
    <div className={`flex flex-col h-full overflow-hidden ${className || ''}`}>
      {/* Tab Bar */}
      <div className="flex bg-bg-sidebar border-b border-gray-800 overflow-x-auto no-scrollbar">
        {tabs.map(tab => (
          <div
            key={tab.id}
            onClick={() => onTabChange(tab.id)}
            className={`flex items-center px-4 h-10 border-r border-gray-800 cursor-pointer min-w-[120px] max-w-[200px] transition-all relative group ${
              activeTabId === tab.id ? 'bg-bg-root text-accent' : 'text-gray-500 hover:bg-bg-root/50 hover:text-gray-300'
            }`}
          >
            <span className="truncate flex-1 text-xs font-bold uppercase tracking-wider">{tab.title}</span>
            {(tab.closable !== false) && (
              <button 
                onClick={(e) => {
                  e.stopPropagation();
                  onTabClose(tab.id);
                }} 
                className={`ml-2 hover:text-white transition-colors p-0.5 rounded hover:bg-white/10 ${
                   activeTabId === tab.id ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
                }`}
                aria-label={`Close ${tab.title}`}
              >
                <X className="h-3 w-3" />
              </button>
            )}
            {activeTabId === tab.id && <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-accent" />}
          </div>
        ))}
      </div>

      {/* Content Area */}
      <div className="flex-1 overflow-hidden relative bg-bg-root">
        {tabs.map(tab => (
            <div 
              key={tab.id} 
              className={`absolute inset-0 w-full h-full ${activeTabId === tab.id ? 'block' : 'hidden'}`}
            >
                {tab.content}
            </div>
        ))}
      </div>
    </div>
  );
};

export default SessionContainer;
