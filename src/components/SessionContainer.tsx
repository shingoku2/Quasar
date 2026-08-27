import React from 'react';
import { X, Columns, Square } from 'lucide-react';

export interface SessionTab {
  id: string;
  title: string;
  content: React.ReactNode;
  closable?: boolean;
}

interface SessionContainerProps {
  tabs: SessionTab[];
  activeTabId: string;
  splitViewIds?: string[]; // IDs of tabs to show in split view
  onTabChange: (id: string) => void;
  onTabClose: (id: string) => void;
  onToggleSplit?: (id: string) => void; // Optional handler to toggle split for a specific tab
  className?: string;
}

const SessionContainer: React.FC<SessionContainerProps> = ({
  tabs,
  activeTabId,
  splitViewIds = [],
  onTabChange,
  onTabClose,
  onToggleSplit,
  className
}) => {
  const isSplitMode = splitViewIds.length > 0;

  // If in split mode, we show all splitViewIds. 
  // If not, we show activeTabId.
  const visibleIds = isSplitMode ? splitViewIds : [activeTabId];

  // Calculate grid columns for split view
  // Simple logic: equal width columns for now
  const gridStyle = isSplitMode ? {
    display: 'grid',
    gridTemplateColumns: `repeat(${visibleIds.length}, 1fr)`,
    height: '100%'
  } : undefined;

  return (
    <div className={`flex flex-col h-full overflow-hidden ${className || ''}`}>
      {/* Tab Bar */}
      <div className="flex bg-bg-sidebar border-b border-gray-800 overflow-x-auto no-scrollbar">
        {tabs.map(tab => (
          <div
            key={tab.id}
            onClick={() => onTabChange(tab.id)}
            className={`flex items-center px-4 h-10 border-r border-gray-800 cursor-pointer min-w-[120px] max-w-[200px] transition-all relative group ${
              visibleIds.includes(tab.id) ? 'bg-bg-root text-accent' : 'text-gray-500 hover:bg-bg-root/50 hover:text-gray-300'
            }`}
          >
            <span className="truncate flex-1 text-xs font-bold uppercase tracking-wider">{tab.title}</span>
            
            {/* Split Toggle Button (Only if NOT the inventory tab and onToggleSplit provided) */}
            {(tab.closable !== false && onToggleSplit) && (
               <button
                 onClick={(e) => {
                   e.stopPropagation();
                   onToggleSplit(tab.id);
                 }}
                 className={`ml-2 hover:text-white transition-colors p-0.5 rounded hover:bg-white/10 ${
                    // Highlight if currently in split view
                    splitViewIds.includes(tab.id) ? 'text-accent' : 'opacity-0 group-hover:opacity-100'
                 }`}
                 title="Toggle Split View"
               >
                 {splitViewIds.includes(tab.id) ? <Square className="h-3 w-3" /> : <Columns className="h-3 w-3" />}
               </button>
            )}

            {(tab.closable !== false) && (
              <button 
                onClick={(e) => {
                  e.stopPropagation();
                  onTabClose(tab.id);
                }} 
                className={`ml-1 hover:text-white transition-colors p-0.5 rounded hover:bg-white/10 ${
                   visibleIds.includes(tab.id) ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
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

      {/* Content Area. Every tab always stays mounted (hidden via `hidden`/display:none
          when not visible) so toggling split view never unmounts a tab that isn't part
          of the split set — an unmount would tear down its live SSH/SFTP session. */}
      <div className="flex-1 overflow-hidden relative bg-bg-root" style={isSplitMode ? gridStyle : undefined}>
        {tabs.map(tab => {
          const isVisible = visibleIds.includes(tab.id);
          return (
            <div
              key={tab.id}
              className={
                isSplitMode
                  ? `relative w-full h-full border-r border-gray-800 last:border-r-0 ${isVisible ? '' : 'hidden'}`
                  : `absolute inset-0 w-full h-full ${isVisible ? 'block' : 'hidden'}`
              }
            >
              {tab.content}
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default SessionContainer;