import React, { useState } from 'react';
import Sidebar, { ViewId } from './Sidebar';
import TopBar from './TopBar';
import RemoteManager from './RemoteManager';
import AIAssistant from './AIAssistant';
import DashboardView from './dashboard/DashboardView';
import MonitoringView from './MonitoringView';
import SecurityView from './vault/SecurityView';
<<<<<<< HEAD
import ScheduledTasksView from './ScheduledTasksView';
=======
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
import SettingsView from './SettingsView';

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
            <DashboardView onNavigate={setActiveView} />
          </div>
          <div className={`absolute inset-0 ${activeView === 'remote' ? 'block' : 'hidden'}`}>
            <RemoteManager />
          </div>
          <div className={`absolute inset-0 ${activeView === 'ai' ? 'block' : 'hidden'}`}>
            <AIAssistant />
          </div>
          <div className={`absolute inset-0 ${activeView === 'monitoring' ? 'block' : 'hidden'}`}>
            <MonitoringView />
          </div>
<<<<<<< HEAD
          <div className={`absolute inset-0 ${activeView === 'automation' ? 'block' : 'hidden'}`}>
            <ScheduledTasksView />
          </div>
=======
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
          <div className={`absolute inset-0 ${activeView === 'security' ? 'block' : 'hidden'}`}>
            <SecurityView />
          </div>
          <div className={`absolute inset-0 ${activeView === 'settings' ? 'block' : 'hidden'}`}>
            <SettingsView />
          </div>
        </main>

        {/* Status Bar */}
        <footer className="h-7 bg-bg-sidebar border-t border-border flex items-center justify-between px-4 text-[11px] text-gray-500 shrink-0">
          <div className="flex items-center space-x-4">
            <div className="flex items-center space-x-1.5">
              <span className="h-1.5 w-1.5 rounded-full bg-success" />
              <span>Connected</span>
            </div>
          </div>
          <div className="flex items-center space-x-4">
            <span>Last scan: --</span>
          </div>
        </footer>
      </div>
    </div>
  );
};

export default Layout;