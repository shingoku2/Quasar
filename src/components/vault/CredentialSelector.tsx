import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Key, X, Search } from 'lucide-react';

interface CredentialSummary {
  id: string;
  name: string;
  username: string;
  credential_type: string;
  host?: string;
  port?: number;
}

interface Credential extends CredentialSummary {
  password: string;
  has_private_key: boolean;
  has_key_passphrase: boolean;
}

interface CredentialSelectorProps {
  hostAddress?: string;
  allowedTypes?: string[];
  onSelect: (credential: Credential) => void;
  onCancel: () => void;
  onManualEntry: () => void;
}

const CredentialSelector: React.FC<CredentialSelectorProps> = ({ 
  hostAddress,
  allowedTypes,
  onSelect, 
  onCancel,
  onManualEntry 
}) => {
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState('');

  useEffect(() => {
    loadCredentials();
  }, []);

  const loadCredentials = async () => {
    setIsLoading(true);
    setError('');
    try {
      let creds = await invoke<CredentialSummary[]>('list_credentials');
      
      if (hostAddress) {
        const types = allowedTypes ?? ['ssh', 'ssh_key'];
        creds = creds.filter(c => 
          types.includes(c.credential_type) && 
          (!c.host || c.host === hostAddress)
        );
      } else if (allowedTypes) {
        creds = creds.filter(c => allowedTypes.includes(c.credential_type));
      }
      
      setCredentials(creds);
    } catch (err) {
      setError(err as string || 'Failed to load credentials');
    } finally {
      setIsLoading(false);
    }
  };

  const handleSelect = async (id: string) => {
    try {
      const cred = await invoke<Credential>('get_credential', { credentialId: id });
      onSelect(cred);
    } catch (err) {
      setError(err as string || 'Failed to retrieve credential');
    }
  };

  const filteredCredentials = credentials.filter(c =>
    c.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    c.username.toLowerCase().includes(searchQuery.toLowerCase()) ||
    (c.host && c.host.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  return (
    <div className="fixed inset-0 z-100 flex items-center justify-center bg-black/80 backdrop-blur-md">
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in duration-200">
        <div className="px-6 py-4 border-b border-gray-800 flex justify-between items-center bg-bg-root/50">
          <div className="flex items-center space-x-2">
            <Key className="h-4 w-4 text-accent" />
            <h2 className="text-sm font-bold text-white uppercase tracking-wider">Select Credential</h2>
          </div>
          <button onClick={onCancel} className="text-gray-500 hover:text-white transition-colors">
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="p-6 space-y-4">
          {hostAddress && (
            <div className="bg-accent/10 border border-accent/30 rounded-lg px-4 py-2 text-sm text-gray-300">
              Connecting to: <span className="text-white font-mono">{hostAddress}</span>
            </div>
          )}

          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500" />
            <input
              type="text"
              placeholder="Search credentials..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full bg-bg-root border border-gray-700 rounded-lg pl-10 pr-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all placeholder:text-gray-600"
            />
          </div>

          {error && (
            <div className="bg-alert/10 border border-alert/30 rounded-lg px-4 py-3 text-alert text-sm">
              {error}
            </div>
          )}

          <div className="max-h-80 overflow-y-auto space-y-2">
            {isLoading ? (
              <div className="text-center text-gray-500 py-8">Loading credentials...</div>
            ) : filteredCredentials.length === 0 ? (
              <div className="text-center text-gray-500 py-8">
                <Key className="h-10 w-10 mx-auto mb-3 opacity-50" />
                <p>No credentials found</p>
                <p className="text-xs mt-1">Add credentials in the Security section</p>
              </div>
            ) : (
              filteredCredentials.map((cred) => (
                <button
                  key={cred.id}
                  onClick={() => handleSelect(cred.id)}
                  className="w-full bg-bg-root border border-gray-700 hover:border-accent rounded-lg p-4 text-left transition-all group"
                >
                  <div className="flex justify-between items-start">
                    <div className="flex-1 min-w-0">
                      <h3 className="text-white font-medium truncate group-hover:text-accent transition-colors">
                        {cred.name}
                      </h3>
                      <p className="text-sm text-gray-500 truncate">{cred.username}</p>
                      {cred.host && (
                        <p className="text-xs text-gray-600 font-mono mt-1 truncate">
                          {cred.host}{cred.port ? `:${cred.port}` : ''}
                        </p>
                      )}
                    </div>
                    <span className="text-xs font-mono uppercase text-accent px-2 py-1 rounded bg-accent/10">
                      {cred.credential_type}
                    </span>
                  </div>
                </button>
              ))
            )}
          </div>

          <div className="pt-2 border-t border-gray-800 flex space-x-2">
            <button
              onClick={onManualEntry}
              className="flex-1 bg-bg-root border border-gray-700 hover:border-accent text-white py-2.5 rounded-lg text-sm font-medium transition-all"
            >
              Enter Manually
            </button>
            <button
              onClick={onCancel}
              className="px-6 py-2.5 text-sm font-medium text-gray-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default CredentialSelector;
