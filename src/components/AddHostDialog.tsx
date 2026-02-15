import React, { useState } from 'react';
import { initDatabase } from '../db';

export interface AddHostInitialValues {
  name?: string;
  address?: string;
  protocol?: 'ssh' | 'rdp';
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
  const [protocol, setProtocol] = useState<'ssh' | 'rdp'>(initialValues?.protocol ?? 'ssh');
  const [port, setPort] = useState(initialValues?.port != null ? String(initialValues.port) : '');
  const [username, setUsername] = useState(initialValues?.username ?? '');
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      // Use initDatabase to ensure tables exist and get connection
      const db = await initDatabase();
      
      const parsedPort = port.trim() ? parseInt(port, 10) : null;
      const portVal = Number.isFinite(parsedPort)
        ? parsedPort
        : (protocol === 'rdp' ? 3389 : 22);
      const now = Math.floor(Date.now() / 1000);
      const existing = await db.select<{ id: string }[]>(
        "SELECT id FROM hosts WHERE address = ? AND protocol = ? AND port = ? LIMIT 1",
        [address, protocol, portVal]
      );

      if (existing.length > 0) {
        await db.execute(
          "UPDATE hosts SET name = ?, username = ?, updated_at = ? WHERE id = ?",
          [name, username || null, now, existing[0].id]
        );
      } else {
        const hostId = typeof crypto !== 'undefined' && 'randomUUID' in crypto
          ? crypto.randomUUID()
          : `host-${now}-${Math.random().toString(36).slice(2, 10)}`;

        await db.execute(
          "INSERT INTO hosts (id, name, address, protocol, port, username, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
          [hostId, name, address, protocol, portVal, username || null, now, now]
        );
      }

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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
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
            <label className="block text-xs font-bold text-gray-400 uppercase mb-1">Friendly Name</label>
            <input 
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
              <label className="block text-xs font-bold text-gray-400 uppercase mb-1">Protocol</label>
              <select 
                className="w-full bg-gray-900 border border-gray-700 rounded px-3 py-2 text-white focus:outline-none focus:border-blue-500"
                value={protocol}
                onChange={(e) => setProtocol(e.target.value as 'ssh' | 'rdp')}
              >
                <option value="ssh">SSH</option>
                <option value="rdp">RDP</option>
              </select>
            </div>
            <div>
              <label className="block text-xs font-bold text-gray-400 uppercase mb-1">Port (Optional)</label>
              <input 
                type="number" 
                className="w-full bg-gray-900 border border-gray-700 rounded px-3 py-2 text-white focus:outline-none focus:border-blue-500"
                placeholder={protocol === 'ssh' ? '22' : '3389'}
                value={port}
                onChange={(e) => setPort(e.target.value)}
              />
            </div>
          </div>
          <div>
            <label className="block text-xs font-bold text-gray-400 uppercase mb-1">Hostname or IP Address</label>
            <input 
              required
              type="text" 
              className="w-full bg-gray-900 border border-gray-700 rounded px-3 py-2 text-white focus:outline-none focus:border-blue-500"
              placeholder="e.g. 192.168.1.100 or myserver.com"
              value={address}
              onChange={(e) => setAddress(e.target.value)}
            />
          </div>
          <div>
            <label className="block text-xs font-bold text-gray-400 uppercase mb-1">Default Username (Optional)</label>
            <input 
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
