import React, { useState, useEffect } from 'react';
import HostList, { Host } from './HostList';
import AddHostDialog from './AddHostDialog';
import AIAssistant from './AIAssistant';
import { initDatabase } from '../db';
import { invoke } from "@tauri-apps/api/core";

interface Tab {
  id: string;
  title: string;
  content: React.ReactNode;
}

const Layout: React.FC = () => {
  const [showAddHost, setShowAddHost] = useState(false);
  const [refreshTrigger, setRefreshTrigger] = useState(0);
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeTabId, setActiveTabId] = useState('dashboard');

  const addTab = (title: string, content: React.ReactNode, id?: string) => {
    // If ID is provided, check if it exists and switch to it
    if (id) {
      const existing = tabs.find(t => t.id === id);
      if (existing) {
        setActiveTabId(id);
        return;
      }
    }
    const newId = id || Math.random().toString(36).substring(7);
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
    // Initialize DB schema
    initDatabase().catch(err => {
      console.error("Failed to initialize database:", err);
      // alert("Database initialization failed. Please check the console.");
    });

    // Set initial dashboard tab
    setTabs([
      { 
        id: 'dashboard', 
        title: 'Dashboard', 
        content: (
          <div className="flex flex-col h-full">
            <div className="p-6 bg-gray-900 border-b border-gray-800">
              <div className="flex justify-between items-center">
                <h1 className="text-2xl font-bold text-white">Host Inventory</h1>
                <button 
                  onClick={() => setShowAddHost(true)}
                  className="bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded text-sm font-bold transition-colors flex items-center"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="12 4v16m8-8H4" />
                  </svg>
                  Add Host
                </button>
              </div>
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
  }, [refreshTrigger]); // Re-render dashboard when refreshTrigger changes (e.g. after adding host)

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
    <div className="flex h-screen bg-gray-900 text-gray-100 overflow-hidden">
      {/* Sidebar */}
      <aside className="w-64 bg-gray-800 border-r border-gray-700 flex flex-col">
        <div className="p-4 border-b border-gray-700 font-bold text-xl tracking-wider text-blue-400">
          TITAN
        </div>
        <nav className="flex-1 overflow-y-auto p-2">
          <ul className="space-y-1">
            <li>
              <button 
                onClick={() => setActiveTabId('dashboard')}
                className={`w-full text-left px-3 py-2 rounded transition-colors ${activeTabId === 'dashboard' ? 'bg-gray-700 text-white' : 'hover:bg-gray-700 text-gray-400'}`}
              >
                Dashboard
              </button>
            </li>
            <li>
              <button 
                onClick={() => addTab('AI Assistant', <AIAssistant />, 'ai-assistant')}
                className={`w-full text-left px-3 py-2 rounded transition-colors ${activeTabId === 'ai-assistant' ? 'bg-gray-700 text-white' : 'hover:bg-gray-700 text-gray-400'}`}
              >
                AI Assistant
              </button>
            </li>
            {/* Future Navigation Items */}
          </ul>
        </nav>
        <div className="p-4 border-t border-gray-700 text-xs text-gray-500">
          v0.1.0-alpha
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col min-w-0">
        {/* Tab Bar */}
        <div className="flex bg-gray-800 border-b border-gray-700 overflow-x-auto no-scrollbar">
          {tabs.map(tab => (
            <div
              key={tab.id}
              onClick={() => setActiveTabId(tab.id)}
              className={`flex items-center px-4 py-2 border-r border-gray-700 cursor-pointer min-w-[120px] max-w-[200px] transition-colors ${
                activeTabId === tab.id ? 'bg-gray-900 text-blue-400 border-b-2 border-b-blue-400' : 'text-gray-400 hover:bg-gray-750'
              }`}
            >
              <span className="truncate flex-1 text-sm">{tab.title}</span>
              {tab.id !== 'dashboard' && (
                <button 
                  onClick={(e) => removeTab(tab.id, e)}
                  className="ml-2 hover:text-white rounded-full p-0.5"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" className="h-3 w-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              )}
            </div>
          ))}
          <button 
            onClick={() => addTab(`New Session ${tabs.length}`, <div className="p-4 text-gray-400 italic">Connecting to host...</div>)}
            className="px-4 py-2 text-gray-400 hover:text-white hover:bg-gray-750 transition-colors"
            title="New Session"
          >
            <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="12 4v16m8-8H4" />
            </svg>
          </button>
        </div>

        {/* Tab Content */}
        <div className="flex-1 overflow-auto bg-gray-900">
          {tabs.find(tab => tab.id === activeTabId)?.content}
        </div>
      </main>

      {showAddHost && (
        <AddHostDialog 
          onClose={() => setShowAddHost(false)} 
          onAdded={() => setRefreshTrigger(prev => prev + 1)} 
        />
      )}
    </div>
  );
};

export default Layout;
