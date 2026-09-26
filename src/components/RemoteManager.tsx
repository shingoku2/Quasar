import React, { useState, useEffect, useRef } from 'react';
import type { Host } from './HostList';
import AddHostDialog, { AddHostInitialValues } from './AddHostDialog';
import SessionContainer, { SessionTab } from './SessionContainer';
import CredentialPrompt from './CredentialPrompt';
import CredentialSelector from './vault/CredentialSelector';
import SshTunnelsView from './SshTunnelsView';
import InventoryPanel from './remote/InventoryPanel';
import SessionContent, { SessionDescriptor } from './remote/SessionContent';
import { useConnectFlow, type OpenSession } from './remote/useConnectFlow';
import { invoke } from "@tauri-apps/api/core";
import { SSH_CREDENTIAL_TYPES, SFTP_CREDENTIAL_TYPES } from '../lib/utils';

const RemoteManager: React.FC = () => {
  const [showAddHost, setShowAddHost] = useState(false);
  const [addHostInitialValues, setAddHostInitialValues] = useState<AddHostInitialValues | undefined>(undefined);
  const [refreshTrigger, setRefreshTrigger] = useState(0);
  // Open sessions as data; tab content is rendered from them below (FE-018).
  const [sessions, setSessions] = useState<SessionDescriptor[]>([]);
  const [activeTabId, setActiveTabId] = useState('inventory');
  const [splitViewIds, setSplitViewIds] = useState<string[]>([]);

  // Track processed quick connect host IDs to prevent duplicate executions
  const processedQuickConnects = useRef(new Set<string>());

  const addSession = (session: SessionDescriptor) => {
    setSessions(prev => [...prev, session]);
    setActiveTabId(session.id);
    // A new session joins an active split view.
    setSplitViewIds(prev => (prev.length > 0 ? [...prev, session.id] : prev));
  };

  const openSession: OpenSession = (mode, host, password, usernameOverride, credentialId) => {
    const username = usernameOverride ?? host.username;
    if (!username) {
      alert(`Username is required to start an ${mode === 'sftp' ? 'SFTP' : 'SSH'} session.`);
      return;
    }
    const uuid = crypto.randomUUID();
    addSession({
      kind: mode,
      id: mode === 'sftp' ? `sftp-${uuid}` : uuid,
      title: `${mode === 'sftp' ? 'SFTP' : 'SSH'}: ${host.name}`,
      address: host.address,
      port: host.port || 22,
      username,
      password,
      credentialId,
    });
  };

  const connectFlow = useConnectFlow(openSession);

  const handleToggleSplit = (id: string) => {
    setSplitViewIds(prev => {
      if (prev.includes(id)) {
        const newSplit = prev.filter(sid => sid !== id);
        return newSplit.length < 2 ? [] : newSplit;
      }
      if (prev.length === 0) {
        return Array.from(new Set([activeTabId, id]));
      }
      return [...prev, id];
    });
  };

  const handleConnect = async (host: Host) => {
    try {
      if (host.protocol === 'ssh') {
        await connectFlow.begin(host, 'ssh');
      } else if (host.protocol === 'rdp') {
        await invoke('connect_rdp', { address: host.address });
        addSession({ kind: 'rdp', id: crypto.randomUUID(), title: `RDP: ${host.name}`, address: host.address });
      } else {
        // database / api / other hosts are inventory + monitoring entries; there is
        // no built-in client to launch for them.
        alert(`No built-in client for "${host.protocol}" hosts. This entry is available for inventory and monitoring.`);
      }
    } catch (error) {
      console.error('Failed to launch session:', error);
      alert(`Failed to launch session: ${error}`);
    }
  };

  const handleSftp = (host: Host) => connectFlow.begin(host, 'sftp');

  const openAddHost = (values?: AddHostInitialValues) => {
    setAddHostInitialValues(values);
    setShowAddHost(true);
  };

  // The quick-connect listener is registered once; it calls the current handler.
  const handleConnectRef = useRef(handleConnect);
  handleConnectRef.current = handleConnect;

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
          void handleConnectRef.current(hostToConnect);
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
  }, []);

  const tabs: SessionTab[] = [
    {
      id: 'inventory',
      title: 'Inventory',
      content: (
        <InventoryPanel
          refreshKey={refreshTrigger}
          onConnect={handleConnect}
          onSftp={handleSftp}
          onAddHost={openAddHost}
        />
      ),
      closable: false,
    },
    { id: 'tunnels', title: 'Tunnels', content: <SshTunnelsView />, closable: false },
    ...sessions.map((session) => ({
      id: session.id,
      title: session.title,
      content: <SessionContent session={session} />,
      closable: true,
    })),
  ];

  const handleTabClose = (id: string) => {
    const remaining = tabs.filter(tab => tab.id !== id);
    setSessions(prev => prev.filter(s => s.id !== id));
    setSplitViewIds(prev => prev.filter(sid => sid !== id));
    if (activeTabId === id) {
      setActiveTabId(remaining[remaining.length - 1].id);
    }
  };

  const {
    pendingHost, pendingMode, showCredentialSelector, useManualEntry, allowCredentialSave, pendingHostIsTailscaleSsh,
  } = connectFlow;

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <SessionContainer
        tabs={tabs}
        activeTabId={activeTabId}
        splitViewIds={splitViewIds}
        onTabChange={setActiveTabId}
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
          allowedTypes={pendingMode === 'sftp' ? SFTP_CREDENTIAL_TYPES : SSH_CREDENTIAL_TYPES}
          onSelect={connectFlow.selectCredential}
          onCancel={connectFlow.cancelSelector}
          onManualEntry={connectFlow.chooseManualEntry}
        />
      )}

      {useManualEntry && pendingHost && (
        <CredentialPrompt
          hostName={pendingHost.name}
          initialUsername={pendingHost.username}
          allowSaveCredential={allowCredentialSave}
          allowNoPassword={pendingMode === 'ssh' && pendingHostIsTailscaleSsh}
          onSubmit={connectFlow.submitManual}
          onCancel={connectFlow.cancelManual}
        />
      )}
    </div>
  );
};

export default RemoteManager;
