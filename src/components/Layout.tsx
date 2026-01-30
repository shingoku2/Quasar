import React, { useState } from 'react';
import Sidebar, { ViewId } from './Sidebar';
import TopBar from './TopBar';
import RemoteManager from './RemoteManager';
import AIAssistant from './AIAssistant';

const Layout: React.FC = () => {
  const [activeView, setActiveView] = useState<ViewId>('dashboard');

  const renderView = () => {
    switch (activeView) {
      case 'dashboard':
        return (
          <div className="p-8 h-full flex flex-col items-center justify-center text-center">
            <h1 className="text-4xl font-black text-white mb-4 italic tracking-tighter">DASHBOARD COMING SOON</h1>
            <p className="text-gray-500 max-w-md">Phase 3 will implement the high-density metric grid and alert feed as specified in the SysAdmin Nexus design.</p>
          </div>
        );
      case 'remote':
        return <RemoteManager />;
      case 'ai':
        return <AIAssistant />;
      case 'monitoring':
      case 'automation':
      case 'settings':
        return (
          <div className="p-12 flex flex-col items-center justify-center opacity-20 grayscale">
            <h2 className="text-2xl font-bold uppercase tracking-widest">{activeView} Module</h2>
            <p className="mt-2 font-mono">Status: Locked (Phase 4+)</p>
          </div>
        );
      default:
        return <div className="p-8 text-white">Select a module from the sidebar.</div>;
    }
  };

  return (
    <div className="flex h-screen bg-bg-root text-gray-100 overflow-hidden font-sans selection:bg-accent/30">
      {/* Sidebar Navigation */}
      <Sidebar activeView={activeView} onViewChange={setActiveView} />

      {/* Main Container */}
      <div className="flex-1 flex flex-col min-w-0 h-full relative">
        {/* Global Top Bar */}
        <TopBar activeView={activeView} />

        {/* Content Container */}
        <main className="flex-1 overflow-hidden">
          {renderView()}
        </main>
      </div>
    </div>
  );
};

export default Layout;