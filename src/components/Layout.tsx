import React, { useState } from 'react';
import Sidebar, { ViewId } from './Sidebar';
import TopBar from './TopBar';
import RemoteManager from './RemoteManager';
import AIAssistant from './AIAssistant';
import DashboardView from './dashboard/DashboardView';
import MonitoringView from './MonitoringView';
import SecurityView from './vault/SecurityView';
import ScheduledTasksView from './ScheduledTasksView';
import SettingsView from './SettingsView';
import UpdateBanner from './UpdateBanner';
import { HostKeyPromptHost } from './vault/HostKeyPromptHost';
import { ViewVisibilityProvider } from '../hooks/useViewVisibility';
import ErrorBoundary from './ErrorBoundary';

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

        <UpdateBanner />

        {/* Content Container - Render all views but hide inactive ones.
            Each view is wrapped in a ViewVisibilityProvider so descendants can
            pause background polling while they are hidden (useVisiblePolling). */}
        <main className="flex-1 overflow-hidden relative">
          <div className={`absolute inset-0 ${activeView === 'dashboard' ? 'block' : 'hidden'}`}>
            <ViewVisibilityProvider visible={activeView === 'dashboard'}>
              <ErrorBoundary scope="Dashboard">
                <DashboardView onNavigate={setActiveView} />
              </ErrorBoundary>
            </ViewVisibilityProvider>
          </div>
          <div className={`absolute inset-0 ${activeView === 'remote' ? 'block' : 'hidden'}`}>
            <ViewVisibilityProvider visible={activeView === 'remote'}>
              <ErrorBoundary scope="Remote">
                <RemoteManager />
              </ErrorBoundary>
            </ViewVisibilityProvider>
          </div>
          <div className={`absolute inset-0 ${activeView === 'ai' ? 'block' : 'hidden'}`}>
            <ViewVisibilityProvider visible={activeView === 'ai'}>
              <ErrorBoundary scope="AI Assistant">
                <AIAssistant />
              </ErrorBoundary>
            </ViewVisibilityProvider>
          </div>
          <div className={`absolute inset-0 ${activeView === 'monitoring' ? 'block' : 'hidden'}`}>
            <ViewVisibilityProvider visible={activeView === 'monitoring'}>
              <ErrorBoundary scope="Monitoring">
                <MonitoringView />
              </ErrorBoundary>
            </ViewVisibilityProvider>
          </div>
          <div className={`absolute inset-0 ${activeView === 'automation' ? 'block' : 'hidden'}`}>
            <ViewVisibilityProvider visible={activeView === 'automation'}>
              <ErrorBoundary scope="Automation">
                <ScheduledTasksView />
              </ErrorBoundary>
            </ViewVisibilityProvider>
          </div>
          <div className={`absolute inset-0 ${activeView === 'security' ? 'block' : 'hidden'}`}>
            <ViewVisibilityProvider visible={activeView === 'security'}>
              <ErrorBoundary scope="Security">
                <SecurityView />
              </ErrorBoundary>
            </ViewVisibilityProvider>
          </div>
          <div className={`absolute inset-0 ${activeView === 'settings' ? 'block' : 'hidden'}`}>
            <ViewVisibilityProvider visible={activeView === 'settings'}>
              <ErrorBoundary scope="Settings">
                <SettingsView />
              </ErrorBoundary>
            </ViewVisibilityProvider>
          </div>
        </main>

        {/* Above every view, so a prompt raised from any view is visible (FE-011). */}
        <HostKeyPromptHost />

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