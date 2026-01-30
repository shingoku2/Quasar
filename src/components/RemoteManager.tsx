import React, { useState, useEffect } from 'react';
import HostList, { Host } from './HostList';
import AddHostDialog from './AddHostDialog';
import TerminalComponent from './TerminalComponent';
import SessionContainer, { SessionTab } from './SessionContainer';
import CredentialPrompt from './CredentialPrompt';
import { initDatabase } from '../db';
import { invoke } from "@tauri-apps/api/core";
import { Plus } from 'lucide-react';

const RemoteManager: React.FC = () => {
  const [showAddHost, setShowAddHost] = useState(false);
  const [refreshTrigger, setRefreshTrigger] = useState(0);
  const [tabs, setTabs] = useState<SessionTab[]>([]);
  const [activeTabId, setActiveTabId] = useState('inventory');
  const [splitViewIds, setSplitViewIds] = useState<string[]>([]);
  
  // State for credential prompt
  const [pendingHost, setPendingHost] = useState<Host | null>(null);

  const addTab = (id: string, title: string, content: React.ReactNode) => {
    setTabs(prev => [...prev, { id, title, content, closable: true }]);
    setActiveTabId(id);
    if (splitViewIds.length > 0) {
      setSplitViewIds(prev => [...prev, id]);
    }
  };

  const handleToggleSplit = (id: string) => {
    setSplitViewIds(prev => {
      if (prev.includes(id)) {
        const newSplit = prev.filter(sid => sid !== id);
        return newSplit.length < 2 ? [] : newSplit;
      } else {
        if (prev.length === 0) {
           const ids = new Set([activeTabId, id]);
           return Array.from(ids);
        }
        return [...prev, id];
      }
    });
  };
  
  const handleTabChange = (id: string) => {
    setActiveTabId(id);
  };

  const startSession = (host: Host, password?: string) => {
    const sessionId = Math.random().toString(36).substring(7);
    addTab(
      sessionId, 
      `SSH: ${host.name}`, 
      <TerminalComponent 
        sessionId={sessionId}
        host={host.address}
        port={host.port || 22}
        username={host.username || 'root'}
        password={password} 
      />
    );
  };

  const handleConnect = async (host: Host) => {
    try {
      if (host.protocol === 'ssh') {
        // Always prompt for password if not provided in host (though we don't store it in host yet)
        setPendingHost(host);
      } else if (host.protocol === 'rdp') {
        await invoke('connect_rdp', { address: host.address });
        const sessionId = Math.random().toString(36).substring(7);
        addTab(sessionId, `RDP: ${host.name}`, <div className="p-10 text-center"><h2 className="text-xl text-blue-400 mb-2">RDP Session Launched</h2><p className="text-gray-400">Launched RDP client for {host.address}</p></div>);
      }
    } catch (error) {
      console.error('Failed to launch session:', error);
      alert(`Failed to launch session: ${error}`);
    }
  };

  useEffect(() => {
    initDatabase().catch(console.error);
    setTabs([
      { 
        id: 'inventory', 
        title: 'Inventory', 
        content: (
          <div className="flex flex-col h-full bg-bg-root">
            <div className="p-6 border-b border-gray-800 flex justify-between items-center">
              <h1 className="text-xl font-bold text-white">Remote Hosts</h1>
              <button 
                onClick={() => setShowAddHost(true)}
                className="bg-accent hover:bg-accent/80 text-white px-4 py-2 rounded-lg text-sm font-bold transition-all flex items-center shadow-lg shadow-accent/10"
              >
                <Plus className="h-4 w-4 mr-2" />
                Add Host
              </button>
            </div>
            <div className="flex-1 overflow-hidden">
              <HostList 
                key={refreshTrigger} 
                onConnect={handleConnect} 
              />
            </div>
          </div>
        ),
        closable: false
      }
    ]);
  }, [refreshTrigger]);

  const handleTabClose = (id: string) => {
    if (tabs.length === 1) return;
    const newTabs = tabs.filter(tab => tab.id !== id);
    setTabs(newTabs);
    setSplitViewIds(prev => prev.filter(sid => sid !== id));
    if (activeTabId === id) {
      setActiveTabId(newTabs[newTabs.length - 1].id);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <SessionContainer 
        tabs={tabs}
        activeTabId={activeTabId}
        splitViewIds={splitViewIds}
        onTabChange={handleTabChange}
        onTabClose={handleTabClose}
        onToggleSplit={handleToggleSplit}
      />

      {showAddHost && (
        <AddHostDialog 
          onClose={() => setShowAddHost(false)} 
          onAdded={() => setRefreshTrigger(prev => prev + 1)} 
        />
      )}

      {pendingHost && (
        <CredentialPrompt 
          hostName={pendingHost.name}
          username={pendingHost.username || 'root'}
          onSubmit={(password) => {
            startSession(pendingHost, password);
            setPendingHost(null);
          }}
          onCancel={() => setPendingHost(null)}
        />
      )}
    </div>
  );
};

export default RemoteManager;
