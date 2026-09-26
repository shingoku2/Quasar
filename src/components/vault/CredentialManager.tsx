import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Key, Plus, Search } from 'lucide-react';
import { getErrorMessage } from '../../lib/utils';
import CredentialCard from './credentials/CredentialCard';
import CredentialFormDialog from './credentials/CredentialFormDialog';
import CredentialViewDialog from './credentials/CredentialViewDialog';
import type { Credential } from './credentials/types';
import { useCredentials } from './credentials/useCredentials';

const CredentialManager: React.FC = () => {
  const { credentials, isLoading, error, setError, load: loadCredentials, search, remove } = useCredentials();
  const [searchQuery, setSearchQuery] = useState('');
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [editingCredential, setEditingCredential] = useState<Credential | null>(null);
  const [selectedCredential, setSelectedCredential] = useState<Credential | null>(null);

  const handleSearch = () => search(searchQuery);

  const handleDelete = async (id: string) => {
    if (!confirm('Are you sure you want to delete this credential?')) return;
    await remove(id);
  };

  const handleView = async (id: string) => {
    try {
      const cred = await invoke<Credential>('get_credential', { credentialId: id });
      setSelectedCredential(cred);
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to retrieve credential'));
    }
  };

  const handleEdit = async (id: string) => {
    try {
      const cred = await invoke<Credential>('get_credential', { credentialId: id });
      setEditingCredential(cred);
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to retrieve credential'));
    }
  };

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-gray-800">
        <div className="flex justify-between items-center mb-4">
          <div className="flex items-center space-x-2">
            <Key className="h-5 w-5 text-accent" />
            <h1 className="text-xl font-bold text-white">Credential Manager</h1>
          </div>
          <button 
            onClick={() => setShowAddDialog(true)}
            className="bg-accent hover:bg-accent/80 text-white px-4 py-2 rounded-lg text-sm font-bold transition-all flex items-center shadow-lg shadow-accent/10"
          >
            <Plus className="h-4 w-4 mr-2" />
            Add Credential
          </button>
        </div>

        <div className="flex space-x-2">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500" />
            <input
              type="text"
              placeholder="Search credentials..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
              className="w-full bg-bg-sidebar border border-gray-700 rounded-lg pl-10 pr-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all placeholder:text-gray-600"
            />
          </div>
          <button
            onClick={handleSearch}
            className="bg-bg-sidebar border border-gray-700 hover:border-accent text-white px-4 py-2 rounded-lg text-sm font-medium transition-all"
          >
            Search
          </button>
          {searchQuery && (
            <button
              onClick={() => {
                setSearchQuery('');
                loadCredentials();
              }}
              className="bg-bg-sidebar border border-gray-700 hover:border-alert text-white px-4 py-2 rounded-lg text-sm font-medium transition-all"
            >
              Clear
            </button>
          )}
        </div>
      </div>

      {error && (
        <div className="mx-6 mt-4 bg-alert/10 border border-alert/30 rounded-lg px-4 py-3 text-alert text-sm" role="alert" aria-live="assertive">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-auto p-6">
        {isLoading ? (
          <div className="text-center text-gray-500 py-12">Loading credentials...</div>
        ) : credentials.length === 0 ? (
          <div className="text-center text-gray-500 py-12">
            <Key className="h-12 w-12 mx-auto mb-4 opacity-50" />
            <p>No credentials stored yet</p>
            <p className="text-sm mt-2">Click "Add Credential" to get started</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {credentials.map((cred) => (
              <CredentialCard
                key={cred.id}
                credential={cred}
                onView={() => handleView(cred.id)}
                onEdit={() => handleEdit(cred.id)}
                onDelete={() => handleDelete(cred.id)}
              />
            ))}
          </div>
        )}
      </div>

      {showAddDialog && (
        <CredentialFormDialog
          onClose={() => setShowAddDialog(false)}
          onSaved={() => {
            setShowAddDialog(false);
            loadCredentials();
          }}
        />
      )}

      {editingCredential && (
        <CredentialFormDialog
          credential={editingCredential}
          onClose={() => setEditingCredential(null)}
          onSaved={() => {
            setEditingCredential(null);
            loadCredentials();
          }}
        />
      )}

      {selectedCredential && (
        <CredentialViewDialog
          credential={selectedCredential}
          onClose={() => setSelectedCredential(null)}
        />
      )}
    </div>
  );
};

export default CredentialManager;
