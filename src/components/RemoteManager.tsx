import React, { useState, useEffect } from 'react';
import HostList, { Host } from './HostList';
import AddHostDialog from './AddHostDialog';
import { initDatabase } from '../db';
import { invoke } from "@tauri-apps/api/core";
import { Plus, X } from 'lucide-react';

interface Tab {
  id: string;
  title: string;
  content: React.ReactNode;
}

const RemoteManager: React.FC = () => {
  const [showAddHost, setShowAddHost] = useState(false);
  const [refreshTrigger, setRefreshTrigger] = useState(0);
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeTabId, setActiveTabId] = useState('inventory');

  const addTab = (title: string, content: React.ReactNode) => {
    const newId = Math.random().toString(36).substring(7);
    setTabs(prev => [...prev, { id: newId, title, content }]);
    setActiveTabId(newId);
  };

  const handleConnect = async (host: Host) => {
    try {
      if (host.protocol === 'ssh') {
        await invoke('connect_ssh', { address: host.address, username: host.username });
        addTab(`SSH: ${host.name}`, <div className="p-10 text-center"><h2 className="text-xl text-green-400 mb-2">SSH Session Launched</h2><p className="text-gray-400">Launched system terminal for {host.address}</p></div>);
      } else if (host.protocol === 'rdp') {
        await invoke('connect_rdp', { address: host.address });
        addTab(`RDP: ${host.name}`, <div className="p-10 text-center"><h2 className="text-xl text-blue-400 mb-2">RDP Session Launched</h2><p className="text-gray-400">Launched RDP client for {host.address}</p></div>);
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
        ) 
      }
    ]);
  }, [refreshTrigger]);

  const removeTab = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (tabs.length === 1) return;
    const newTabs = tabs.filter(tab => tab.id !== id);
    setTabs(newTabs);
    if (activeTabId === id) {
      setActiveTabId(newTabs[newTabs.length - 1].id);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Tab Bar */}
      <div className="flex bg-bg-sidebar border-b border-gray-800 overflow-x-auto no-scrollbar">
        {tabs.map(tab => (
          <div
            key={tab.id}
            onClick={() => setActiveTabId(tab.id)}
            className={`flex items-center px-4 h-10 border-r border-gray-800 cursor-pointer min-w-[120px] max-w-[200px] transition-all relative ${
              activeTabId === tab.id ? 'bg-bg-root text-accent' : 'text-gray-500 hover:bg-bg-root/50 hover:text-gray-300'
            }`}
          >
            <span className="truncate flex-1 text-xs font-bold uppercase tracking-wider">{tab.title}</span>
            {tab.id !== 'inventory' && (
              <button onClick={(e) => removeTab(tab.id, e)} className="ml-2 hover:text-white transition-colors">
                <X className="h-3 w-3" />
              </button>
            )}
            {activeTabId === tab.id && <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-accent" />}
          </div>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {tabs.find(tab => tab.id === activeTabId)?.content}
      </div>

      {showAddHost && (
        <AddHostDialog 
          onClose={() => setShowAddHost(false)} 
          onAdded={() => setRefreshTrigger(prev => prev + 1)} 
        />
      )}
    </div>
  );
};

export default RemoteManager;
