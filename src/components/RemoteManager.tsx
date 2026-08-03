import React, { useState, useEffect, useRef, Suspense } from 'react';
import HostList, { Host } from './HostList';
import AddHostDialog, { AddHostInitialValues } from './AddHostDialog';
// xterm.js is ~333 kB of the bundle and is only needed once a user actually opens
// an SSH session, so it is split into its own chunk and loaded on demand.
const TerminalComponent = React.lazy(() => import('./TerminalComponent'));
import SshFileManager from './SshFileManager';
import SessionContainer, { SessionTab } from './SessionContainer';
import CredentialPrompt from './CredentialPrompt';
import CredentialSelector from './vault/CredentialSelector';
import SshHostKeyPrompt from './vault/SshHostKeyPrompt';
import SshTunnelsView from './SshTunnelsView';
import { useSshHostKeyVerification } from '../hooks/useSshHostKeyVerification';
import { invoke } from "@tauri-apps/api/core";
import { Plus } from 'lucide-react';

interface Credential {
  id: string;
  name: string;
  username: string;
  password: string;
  credential_type: string;
  host?: string;
  port?: number;
}

const RemoteManager: React.FC = () => {
  const [showAddHost, setShowAddHost] = useState(false);
  const [addHostInitialValues, setAddHostInitialValues] = useState<AddHostInitialValues | undefined>(undefined);
  const [refreshTrigger, setRefreshTrigger] = useState(0);
  const [tabs, setTabs] = useState<SessionTab[]>([]);
  const [activeTabId, setActiveTabId] = useState('inventory');
  const [splitViewIds, setSplitViewIds] = useState<string[]>([]);
  
  // State for credential prompt and selector
  const [pendingHost, setPendingHost] = useState<Host | null>(null);
  const [pendingMode, setPendingMode] = useState<'ssh' | 'sftp'>('ssh');
  const [showCredentialSelector, setShowCredentialSelector] = useState(false);
  const [useManualEntry, setUseManualEntry] = useState(false);
  const [allowCredentialSave, setAllowCredentialSave] = useState(false);
  
  // Track processed quick connect host IDs to prevent duplicate executions
  const processedQuickConnects = useRef(new Set<string>());
  
  // SSH host key verification
  const { promptData, handleTrust, handleReject } = useSshHostKeyVerification();

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

  const startSession = (host: Host, password?: string, usernameOverride?: string, credentialId?: string) => {
    const sessionUsername = usernameOverride ?? host.username;
    if (!sessionUsername) {
      alert('Username is required to start an SSH session.');
      return;
    }

    const sessionId = Math.random().toString(36).substring(7);
    addTab(
      sessionId, 
      `SSH: ${host.name}`, 
      <Suspense fallback={<div className="p-4 text-gray-400 text-sm">Loading terminal…</div>}>
        <TerminalComponent
          sessionId={sessionId}
          host={host.address}
          port={host.port || 22}
          username={sessionUsername}
          password={password}
          credentialId={credentialId}
        />
      </Suspense>
    );
  };

  const startSftpSession = (host: Host, password?: string, usernameOverride?: string) => {
    const sessionUsername = usernameOverride ?? host.username;
    if (!sessionUsername) {
      alert('Username is required to start an SFTP session.');
      return;
    }

    const sessionId = `sftp-${Math.random().toString(36).substring(7)}`;
    addTab(
      sessionId, 
      `SFTP: ${host.name}`, 
      <SshFileManager 
        host={host.address}
        port={host.port || 22}
        username={sessionUsername}
        password={password} 
      />
    );
  };

  const handleConnect = async (host: Host) => {
    try {
      if (host.protocol === 'ssh') {
        setPendingHost(host);
        setPendingMode('ssh');
        setUseManualEntry(false);
        
        try {
          const isLocked = await invoke<boolean>('is_vault_locked');
          if (isLocked) {
            setAllowCredentialSave(false);
            setUseManualEntry(true);
          } else {
            setShowCredentialSelector(true);
          }
        } catch {
          setAllowCredentialSave(false);
          setUseManualEntry(true);
        }
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

  const handleSftp = async (host: Host) => {
    try {
      setPendingHost(host);
      setPendingMode('sftp');
      setUseManualEntry(false);
      
      try {
        const isLocked = await invoke<boolean>('is_vault_locked');
        if (isLocked) {
          setAllowCredentialSave(false);
          setUseManualEntry(true);
        } else {
          setShowCredentialSelector(true);
        }
      } catch {
        setAllowCredentialSave(false);
        setUseManualEntry(true);
      }
    } catch (error) {
      console.error('Failed to launch SFTP:', error);
      alert(`Failed to launch SFTP: ${error}`);
    }
  };

  const handleCredentialSelected = (credential: Credential) => {
    if (pendingHost) {
      const selectedUsername = credential.username || pendingHost.username;
      if (!selectedUsername) {
        setShowCredentialSelector(false);
        setUseManualEntry(true);
        return;
      }

      if (pendingMode === 'sftp') {
        startSftpSession(pendingHost, credential.password, selectedUsername);
      } else {
        // Pass credentialId only — the backend fetches the credential from the vault
        // by ID, so the password never needs to cross the IPC boundary for SSH sessions.
        startSession(pendingHost, undefined, selectedUsername, credential.id);
      }
      setShowCredentialSelector(false);
      setPendingHost(null);
    }
  };

  const handleManualEntry = () => {
    setShowCredentialSelector(false);
    setAllowCredentialSave(true);
    setUseManualEntry(true);
  };

  // Check for quick connect via event-driven approach (no polling)
  useEffect(() => {
    const processQuickConnect = () => {
      const quickConnectData = sessionStorage.getItem('quickConnectHost');
      if (!quickConnectData) return;

      try {
        const host = JSON.parse(quickConnectData);
        // Deduplicate by host ID only (not timestamp)
        if (processedQuickConnects.current.has(host.id)) return;

        // Clear the stored data immediately
        sessionStorage.removeItem('quickConnectHost');
        // Mark as processed
        processedQuickConnects.current.add(host.id);

        console.log('Quick Connect: Triggering connection to', host.name);

        const hostToConnect = {
          id: host.id,
          name: host.name,
          address: host.address,
          protocol: host.protocol,
          port: host.port || 22,
          username: host.username || undefined
        };

        // Trigger connection after a short delay to ensure tabs are set
        setTimeout(() => {
          handleConnect(hostToConnect);
          // Allow re-connecting to the same host after processing
          processedQuickConnects.current.delete(host.id);
        }, 200);
      } catch (err) {
        console.error('Failed to parse quick connect host:', err);
      }
    };

    // Check once on mount (in case data was set before this component rendered)
    processQuickConnect();

    // Listen for cross-window storage events
    const onStorage = (e: StorageEvent) => {
      if (e.key === 'quickConnectHost' && e.newValue) {
        processQuickConnect();
      }
    };
    window.addEventListener('storage', onStorage);

    // Listen for same-window custom event (storage event doesn't fire in same window)
    const onQuickConnect = () => processQuickConnect();
    window.addEventListener('quickConnectTriggered', onQuickConnect);

    return () => {
      window.removeEventListener('storage', onStorage);
      window.removeEventListener('quickConnectTriggered', onQuickConnect);
    };
  }, [])

  useEffect(() => {
    setTabs([
      { 
        id: 'inventory', 
        title: 'Inventory', 
        content: (
          <div className="flex flex-col h-full bg-bg-root">
            <div className="p-6 border-b border-gray-800 flex justify-between items-center">
              <h1 className="text-xl font-bold text-white">Remote Hosts</h1>
              <button 
                onClick={() => {
                  setAddHostInitialValues(undefined);
                  setShowAddHost(true);
                }}
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
                onSftp={handleSftp}
                onAddHost={(values) => {
                  setAddHostInitialValues(values);
                  setShowAddHost(true);
                }}
              />
            </div>
          </div>
        ),
        closable: false
      },
      {
        id: 'tunnels',
        title: 'Tunnels',
        content: <SshTunnelsView />,
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
          initialValues={addHostInitialValues}
          onClose={() => {
            setShowAddHost(false);
            setAddHostInitialValues(undefined);
          }} 
          onAdded={() => {
            setRefreshTrigger(prev => prev + 1);
            setAddHostInitialValues(undefined);
          }} 
        />
      )}

      {showCredentialSelector && pendingHost && (
        <CredentialSelector
          hostAddress={pendingHost.address}
          allowedTypes={pendingMode === 'sftp' ? ['ssh'] : undefined}
          onSelect={handleCredentialSelected}
          onCancel={() => {
            setShowCredentialSelector(false);
            setPendingHost(null);
          }}
          onManualEntry={handleManualEntry}
        />
      )}

      {useManualEntry && pendingHost && (
        <CredentialPrompt 
          hostName={pendingHost.name}
          initialUsername={pendingHost.username}
          allowSaveCredential={allowCredentialSave}
          onSubmit={async (enteredUsername, password, options) => {
            if (pendingMode === 'sftp') {
              startSftpSession(pendingHost, password, enteredUsername);
            } else {
              startSession(pendingHost, password, enteredUsername);
            }

            if (options?.saveCredential) {
              const credentialName = options.credentialName || `${pendingHost.name} (${enteredUsername})`;
              try {
                await invoke('add_credential', {
                  name: credentialName,
                  username: enteredUsername,
                  password,
                  credentialType: 'ssh',
                  host: pendingHost.address,
                  port: pendingHost.port || 22,
                  metadata: null,
                });
              } catch (error) {
                console.error('Failed to save credential to vault:', error);
                alert(`Connected, but failed to save credential: ${error}`);
              }
            }

            setPendingHost(null);
            setUseManualEntry(false);
            setAllowCredentialSave(false);
          }}
          onCancel={() => {
            setPendingHost(null);
            setUseManualEntry(false);
            setAllowCredentialSave(false);
          }}
        />
      )}

      {promptData && (
        <SshHostKeyPrompt
          host={promptData.host}
          port={promptData.port}
          fingerprint={promptData.fingerprint}
          keyType={promptData.keyType}
          isChanged={promptData.isChanged}
          oldFingerprint={promptData.oldFingerprint}
          onTrust={handleTrust}
          onReject={handleReject}
        />
      )}
    </div>
  );
};

export default RemoteManager;
