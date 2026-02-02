import React, { useState } from 'react';
import Sidebar, { ViewId } from './Sidebar';
import TopBar from './TopBar';
import RemoteManager from './RemoteManager';
import AIAssistant from './AIAssistant';
import DashboardView from './dashboard/DashboardView';
import Automation from './Automation';
import MonitoringView from './MonitoringView';
import SecurityView from './vault/SecurityView';

const Layout: React.FC = () => {
  const [activeView, setActiveView] = useState<ViewId>('dashboard');

  return (
    <div className="flex h-screen bg-bg-root text-gray-100 overflow-hidden font-sans selection:bg-accent/30">
      {/* Sidebar Navigation */}
      <Sidebar activeView={activeView} onViewChange={setActiveView} />

      {/* Main Container */}
      <div className="flex-1 flex flex-col min-w-0 h-full relative">
        {/* Global Top Bar */}
        <TopBar activeView={activeView} />

        {/* Content Container - Render all views but hide inactive ones */}
        <main className="flex-1 overflow-hidden relative">
          <div className={`absolute inset-0 ${activeView === 'dashboard' ? 'block' : 'hidden'}`}>
            <DashboardView />
          </div>
          <div className={`absolute inset-0 ${activeView === 'remote' ? 'block' : 'hidden'}`}>
            <RemoteManager />
          </div>
          <div className={`absolute inset-0 ${activeView === 'ai' ? 'block' : 'hidden'}`}>
            <AIAssistant />
          </div>
          <div className={`absolute inset-0 ${activeView === 'automation' ? 'block' : 'hidden'}`}>
            <Automation />
          </div>
          <div className={`absolute inset-0 ${activeView === 'monitoring' ? 'block' : 'hidden'}`}>
            <MonitoringView />
          </div>
          <div className={`absolute inset-0 ${activeView === 'security' ? 'block' : 'hidden'}`}>
            <SecurityView />
          </div>
          <div className={`absolute inset-0 ${activeView === 'settings' ? 'block' : 'hidden'}`}>
            <div className="p-12 flex flex-col items-center justify-center opacity-20 grayscale">
              <h2 className="text-2xl font-bold uppercase tracking-widest">Settings Module</h2>
              <p className="mt-2 font-mono">Status: Locked (Phase 4+)</p>
            </div>
          </div>
        </main>
      </div>
    </div>
  );
};

export default Layout;