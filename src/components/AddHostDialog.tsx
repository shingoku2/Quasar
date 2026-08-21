import React, { useState } from 'react';
import { invoke } from "@tauri-apps/api/core";

export type HostProtocol = 'ssh' | 'rdp' | 'database' | 'api' | 'other';

const PROTOCOL_OPTIONS: { value: HostProtocol; label: string }[] = [
  { value: 'ssh', label: 'SSH' },
  { value: 'rdp', label: 'RDP' },
  { value: 'database', label: 'Database' },
  { value: 'api', label: 'API' },
  { value: 'other', label: 'Other' },
];

/** Port hint per protocol; shown as the input placeholder. */
const PORT_PLACEHOLDERS: Record<HostProtocol, string> = {
  ssh: '22',
  rdp: '3389',
  database: '5432',
  api: '443',
  other: 'e.g. 8080',
};

/** Only SSH and RDP have backend port defaults; other protocols need an explicit port. */
const PROTOCOLS_WITH_DEFAULT_PORT: HostProtocol[] = ['ssh', 'rdp'];

export interface AddHostInitialValues {
  name?: string;
  address?: string;
  protocol?: HostProtocol;
  port?: number | null;
  username?: string;
}

interface AddHostDialogProps {
  onClose: () => void;
  onAdded: () => void;
  initialValues?: AddHostInitialValues;
}

const AddHostDialog: React.FC<AddHostDialogProps> = ({ onClose, onAdded, initialValues }) => {
  const [name, setName] = useState(initialValues?.name ?? '');
  const [address, setAddress] = useState(initialValues?.address ?? '');
  const [protocol, setProtocol] = useState<HostProtocol>(initialValues?.protocol ?? 'ssh');
  const [port, setPort] = useState(initialValues?.port != null ? String(initialValues.port) : '');
  const [username, setUsername] = useState(initialValues?.username ?? '');
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      const parsedPort = port.trim() ? parseInt(port, 10) : undefined;
      const portVal = Number.isFinite(parsedPort) ? parsedPort : undefined;
      await invoke('upsert_saved_host', {
        name,
        address,
        protocol,
        port: portVal,
        username: username || null,
      });

      window.dispatchEvent(new Event('hostsUpdated'));
      onAdded();
      onClose();
    } catch (err) {
      console.error("Failed to add host:", err);
      // Show the actual error to the user for debugging
      alert(`Failed to add host: ${err}`);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-gray-800 border border-gray-700 rounded-lg shadow-xl w-full max-w-md overflow-hidden">
        <div className="px-6 py-4 border-b border-gray-700 flex justify-between items-center">
          <h2 className="text-lg font-bold text-white">Add New Host</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white">
            <svg xmlns="http://www.w3.org/2000/svg" className="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          <div>
            <label htmlFor="add-host-name" className="block text-xs font-bold text-gray-400 uppercase mb-1">Friendly Name</label>
            <input
              id="add-host-name"
              required
              type="text"
              className="w-full bg-gray-900 border border-gray-700 rounded px-3 py-2 text-white focus:outline-none focus:border-blue-500"
              placeholder="e.g. Production Web Server"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label htmlFor="add-host-protocol" className="block text-xs font-bold text-gray-400 uppercase mb-1">Protocol</label>
              <select
                id="add-host-protocol"
                className="w-full bg-gray-900 border border-gray-700 rounded px-3 py-2 text-white focus:outline-none focus:border-blue-500"
                value={protocol}
                // Safe: the select only renders PROTOCOL_OPTIONS values, all of type HostProtocol.
                onChange={(e) => setProtocol(e.target.value as HostProtocol)}
              >
                {PROTOCOL_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>{opt.label}</option>
                ))}
              </select>
            </div>
            <div>
              <label htmlFor="add-host-port" className="block text-xs font-bold text-gray-400 uppercase mb-1">
                {PROTOCOLS_WITH_DEFAULT_PORT.includes(protocol) ? 'Port (Optional)' : 'Port'}
              </label>
              <input
                id="add-host-port"
                type="number"
                required={!PROTOCOLS_WITH_DEFAULT_PORT.includes(protocol)}
                className="w-full bg-gray-900 border border-gray-700 rounded px-3 py-2 text-white focus:outline-none focus:border-blue-500"
                placeholder={PORT_PLACEHOLDERS[protocol]}
                value={port}
                onChange={(e) => setPort(e.target.value)}
              />
            </div>
          </div>
          <div>
            <label htmlFor="add-host-address" className="block text-xs font-bold text-gray-400 uppercase mb-1">Hostname or IP Address</label>
            <input
              id="add-host-address"
              required
              type="text"
              className="w-full bg-gray-900 border border-gray-700 rounded px-3 py-2 text-white focus:outline-none focus:border-blue-500"
              placeholder="e.g. 192.168.1.100 or myserver.com"
              value={address}
              onChange={(e) => setAddress(e.target.value)}
            />
          </div>
          <div>
            <label htmlFor="add-host-username" className="block text-xs font-bold text-gray-400 uppercase mb-1">Default Username (Optional)</label>
            <input
              id="add-host-username"
              type="text"
              className="w-full bg-gray-900 border border-gray-700 rounded px-3 py-2 text-white focus:outline-none focus:border-blue-500"
              placeholder="e.g. admin"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          <div className="pt-4 flex justify-end space-x-3">
            <button 
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm font-medium text-gray-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button 
              type="submit"
              disabled={submitting}
              className="bg-blue-600 hover:bg-blue-500 disabled:bg-blue-800 disabled:text-gray-400 px-6 py-2 rounded text-sm font-bold text-white transition-colors"
            >
              {submitting ? 'Saving...' : 'Save Host'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export default AddHostDialog;
