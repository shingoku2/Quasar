import React from 'react';
import { Plus } from 'lucide-react';
import HostList, { Host } from '../HostList';
import type { AddHostInitialValues } from '../AddHostDialog';

/** The Inventory tab: saved hosts with connect / SFTP actions and "Add Host". */
const InventoryPanel: React.FC<{
  /** Bumped after a host is added, to reload the list. */
  refreshKey: number;
  onConnect: (host: Host) => void;
  onSftp: (host: Host) => void;
  onAddHost: (values?: AddHostInitialValues) => void;
}> = ({ refreshKey, onConnect, onSftp, onAddHost }) => (
  <div className="flex flex-col h-full bg-bg-root">
    <div className="p-6 border-b border-gray-800 flex justify-between items-center">
      <h1 className="text-xl font-bold text-white">Remote Hosts</h1>
      <button
        onClick={() => onAddHost(undefined)}
        className="bg-accent hover:bg-accent/80 text-white px-4 py-2 rounded-lg text-sm font-bold transition-all flex items-center shadow-lg shadow-accent/10"
      >
        <Plus className="h-4 w-4 mr-2" />
        Add Host
      </button>
    </div>
    <div className="flex-1 overflow-hidden">
      <HostList key={refreshKey} onConnect={onConnect} onSftp={onSftp} onAddHost={onAddHost} />
    </div>
  </div>
);

export default InventoryPanel;
