import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Key, Plus, Edit2, Trash2, Search, Server, User, Lock, Save, X, Eye, EyeOff } from 'lucide-react';

interface CredentialSummary {
  id: string;
  name: string;
  username: string;
  credential_type: string;
  host?: string;
  port?: number;
  created_at: string;
  last_used_at?: string;
}

interface Credential extends CredentialSummary {
  password: string;
}

interface CredentialFormData {
  name: string;
  username: string;
  password: string;
  credential_type: string;
  host: string;
  port: number;
  key_path: string;
  private_key: string;
  key_passphrase: string;
}

function getErrorMessage(err: unknown, fallback: string): string {
  if (err instanceof Error) return err.message;
  if (typeof err === 'string') return err;
  if (err && typeof err === 'object' && 'message' in err && typeof (err as { message: unknown }).message === 'string') {
    return (err as { message: string }).message;
  }
  return fallback;
}

const CredentialManager: React.FC = () => {
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState('');
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [editingCredential, setEditingCredential] = useState<Credential | null>(null);
  const [selectedCredential, setSelectedCredential] = useState<Credential | null>(null);

  const loadCredentials = async () => {
    setIsLoading(true);
    setError('');
    try {
      const creds = await invoke<CredentialSummary[]>('list_credentials');
      setCredentials(creds);
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to load credentials'));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadCredentials();
  }, []);

  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      loadCredentials();
      return;
    }
    setIsLoading(true);
    try {
      const results = await invoke<CredentialSummary[]>('search_credentials', { query: searchQuery });
      setCredentials(results);
    } catch (err) {
      setError(getErrorMessage(err, 'Search failed'));
    } finally {
      setIsLoading(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Are you sure you want to delete this credential?')) return;
    
    try {
      await invoke('delete_credential', { credentialId: id });
      await loadCredentials();
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to delete credential'));
    }
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
        <CredentialDialog
          onClose={() => setShowAddDialog(false)}
          onSaved={() => {
            setShowAddDialog(false);
            loadCredentials();
          }}
        />
      )}

      {editingCredential && (
        <CredentialDialog
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

const CredentialCard: React.FC<{
  credential: CredentialSummary;
  onView: () => void;
  onEdit: () => void;
  onDelete: () => void;
}> = ({ credential, onView, onEdit, onDelete }) => {
  const typeColors: Record<string, string> = {
    ssh: 'text-blue-400',
    rdp: 'text-purple-400',
    database: 'text-green-400',
    api: 'text-yellow-400',
    other: 'text-gray-400',
  };

  return (
    <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-4 hover:border-accent/50 transition-all">
      <div className="flex justify-between items-start mb-3">
        <div className="flex-1 min-w-0">
          <h3 className="text-white font-bold truncate">{credential.name}</h3>
          <p className="text-sm text-gray-500 truncate">{credential.username}</p>
        </div>
        <span className={`text-xs font-mono uppercase px-2 py-1 rounded ${typeColors[credential.credential_type] || typeColors.other}`}>
          {credential.credential_type}
        </span>
      </div>

      {credential.host && (
        <div className="flex items-center text-sm text-gray-400 mb-3">
          <Server className="h-3 w-3 mr-1" />
          <span className="truncate">{credential.host}{credential.port ? `:${credential.port}` : ''}</span>
        </div>
      )}

      <div className="flex justify-end space-x-2 pt-3 border-t border-gray-800">
        <button
          onClick={onView}
          className="text-gray-400 hover:text-accent transition-colors p-1"
          title="View"
        >
          <Eye className="h-4 w-4" />
        </button>
        <button
          onClick={onEdit}
          className="text-gray-400 hover:text-accent transition-colors p-1"
          title="Edit"
        >
          <Edit2 className="h-4 w-4" />
        </button>
        <button
          onClick={onDelete}
          className="text-gray-400 hover:text-alert transition-colors p-1"
          title="Delete"
        >
          <Trash2 className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
};

const CredentialDialog: React.FC<{
  credential?: Credential;
  onClose: () => void;
  onSaved: () => void;
}> = ({ credential, onClose, onSaved }) => {
  const [formData, setFormData] = useState<CredentialFormData>({
    name: credential?.name || '',
    username: credential?.username || '',
    password: credential?.password || '',
    credential_type: credential?.credential_type || 'ssh',
    host: credential?.host || '',
    port: credential?.port || 22,
    key_path: (credential as { key_path?: string })?.key_path || '',
    private_key: (credential as { private_key?: string })?.private_key || '',
    key_passphrase: (credential as { key_passphrase?: string })?.key_passphrase || '',
  });
  const [showPassword, setShowPassword] = useState(false);
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSaving(true);
    setError('');

    if (formData.credential_type === 'ssh_key' && !formData.key_path.trim() && !formData.private_key.trim()) {
      setError('Provide either key path or paste private key PEM.');
      setIsSaving(false);
      return;
    }

    try {
      if (credential) {
        const metadata = JSON.stringify({
          credential_type: formData.credential_type,
          host: formData.host || null,
          port: formData.port || null,
        });
        const payload: Record<string, unknown> = {
          credentialId: credential.id,
          name: formData.name,
          username: formData.username,
          metadata,
          credential_type: formData.credential_type,
        };
        if (formData.credential_type === 'ssh_key') {
          // Send actual values so empty string clears fields on the backend (null would skip update)
          payload.key_path = formData.key_path;
          payload.private_key = formData.private_key;
          payload.key_passphrase = formData.key_passphrase;
          // Clear password when switching to SSH key so stale password is not left in DB
          payload.password = '';
        } else {
          payload.password = formData.password || null;
          // Clear SSH key fields when switching to password-based so stale key data is not left in DB
          payload.key_path = '';
          payload.private_key = '';
          payload.key_passphrase = '';
        }
        await invoke('update_credential', payload);
      } else {
        const isKey = formData.credential_type === 'ssh_key';
        await invoke('add_credential', {
          name: formData.name,
          username: formData.username,
          password: isKey ? '' : formData.password,
          credentialType: formData.credential_type,
          host: formData.host || null,
          port: formData.port ? Number(formData.port) : null,
          metadata: null,
          key_path: isKey && formData.key_path ? formData.key_path : null,
          private_key: isKey && formData.private_key ? formData.private_key : null,
          key_passphrase: isKey && formData.key_passphrase ? formData.key_passphrase : null,
        });
      }
      onSaved();
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to save credential'));
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-100 flex items-center justify-center bg-black/80 backdrop-blur-md" role="dialog" aria-modal="true" aria-label={credential ? 'Edit Credential' : 'Add Credential'}>
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in duration-200">
        <div className="px-6 py-4 border-b border-gray-800 flex justify-between items-center bg-bg-root/50">
          <div className="flex items-center space-x-2">
            {credential ? <Edit2 className="h-4 w-4 text-accent" /> : <Plus className="h-4 w-4 text-accent" />}
            <h2 className="text-sm font-bold text-white uppercase tracking-wider">
              {credential ? 'Edit Credential' : 'Add Credential'}
            </h2>
          </div>
          <button onClick={onClose} className="text-gray-500 hover:text-white transition-colors">
            <X className="h-4 w-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {error && (
            <div className="bg-alert/10 border border-alert/30 rounded-lg px-4 py-3 text-alert text-sm" role="alert" aria-live="assertive">
              {error}
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-gray-400 mb-2">Name</label>
            <input
              type="text"
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
              placeholder="My Server"
              required
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-400 mb-2">Type</label>
            <select
              value={formData.credential_type}
              onChange={(e) => setFormData({ ...formData, credential_type: e.target.value })}
              className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
            >
              <option value="ssh">SSH (password)</option>
              <option value="ssh_key">SSH Key</option>
              <option value="rdp">RDP</option>
              <option value="database">Database</option>
              <option value="api">API</option>
              <option value="other">Other</option>
            </select>
          </div>

          {formData.credential_type === 'ssh_key' && (
            <>
              <div>
                <label className="block text-sm font-medium text-gray-400 mb-2">Key path (optional)</label>
                <input
                  type="text"
                  value={formData.key_path}
                  onChange={(e) => setFormData({ ...formData, key_path: e.target.value })}
                  className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent"
                  placeholder="~/.ssh/id_ed25519"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-400 mb-2">Private key PEM (or use path above)</label>
                <textarea
                  value={formData.private_key}
                  onChange={(e) => setFormData({ ...formData, private_key: e.target.value })}
                  className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent font-mono text-xs min-h-[120px]"
                  placeholder="-----BEGIN OPENSSH PRIVATE KEY-----..."
                  rows={5}
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-400 mb-2">Key passphrase (optional)</label>
                <div className="relative">
                  <input
                    type={showPassphrase ? 'text' : 'password'}
                    autoComplete="off"
                    value={formData.key_passphrase}
                    onChange={(e) => setFormData({ ...formData, key_passphrase: e.target.value })}
                    className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 pr-12 py-2 text-white focus:outline-none focus:border-accent"
                    placeholder="Passphrase for encrypted key"
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassphrase(!showPassphrase)}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300 transition-colors"
                    tabIndex={-1}
                  >
                    {showPassphrase ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                  </button>
                </div>
              </div>
            </>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-400 mb-2">Host (optional)</label>
              <input
                type="text"
                value={formData.host}
                onChange={(e) => setFormData({ ...formData, host: e.target.value })}
                className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                placeholder="192.168.1.100"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-400 mb-2">Port (optional)</label>
              <input
                type="number"
                value={formData.port}
                onChange={(e) => { const v = parseInt(e.target.value, 10); setFormData({ ...formData, port: Number.isFinite(v) && v >= 1 && v <= 65535 ? v : 22 }); }}
                className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                placeholder="22"
              />
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-400 mb-2">Username</label>
            <div className="relative">
              <User className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500" />
              <input
                type="text"
                value={formData.username}
                onChange={(e) => setFormData({ ...formData, username: e.target.value })}
                className="w-full bg-bg-root border border-gray-700 rounded-lg pl-10 pr-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                placeholder="root"
                required
              />
            </div>
          </div>

          {formData.credential_type !== 'ssh_key' && (
          <div>
            <label className="block text-sm font-medium text-gray-400 mb-2">Password</label>
            <div className="relative">
              <Lock className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500" />
              <input
                type={showPassword ? 'text' : 'password'}
                autoComplete="new-password"
                value={formData.password}
                onChange={(e) => setFormData({ ...formData, password: e.target.value })}
                className="w-full bg-bg-root border border-gray-700 rounded-lg pl-10 pr-12 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                placeholder="••••••••"
                required
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300 transition-colors"
                tabIndex={-1}
              >
                {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
          </div>
          )}

          <div className="pt-2 flex space-x-2">
            <button
              type="submit"
              disabled={isSaving}
              className="flex-1 bg-accent hover:bg-accent/80 disabled:bg-accent/50 disabled:cursor-not-allowed text-white py-2.5 rounded-lg text-sm font-bold transition-all shadow-lg shadow-accent/20 flex items-center justify-center"
            >
              <Save className="h-4 w-4 mr-2" />
              {isSaving ? 'Saving...' : 'Save'}
            </button>
            <button
              type="button"
              onClick={onClose}
              disabled={isSaving}
              className="px-6 py-2.5 text-sm font-medium text-gray-400 hover:text-white transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

const CredentialViewDialog: React.FC<{
  credential: Credential;
  onClose: () => void;
}> = ({ credential, onClose }) => {
  const [showPassword, setShowPassword] = useState(false);
  const [copied, setCopied] = useState(false);

  const clipboardClearTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);

  const copyToClipboard = async (text: string) => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    // Clear the copied state indicator after 2 s.
    setTimeout(() => setCopied(false), 2000);
    // Auto-clear clipboard after 30 s to prevent credential exposure.
    if (clipboardClearTimerRef.current !== null) {
      clearTimeout(clipboardClearTimerRef.current);
    }
    clipboardClearTimerRef.current = setTimeout(() => {
      navigator.clipboard.writeText('').catch(() => {});
      clipboardClearTimerRef.current = null;
    }, 30000);
  };

  return (
    <div className="fixed inset-0 z-100 flex items-center justify-center bg-black/80 backdrop-blur-md" role="dialog" aria-modal="true" aria-label="View Credential">
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-md overflow-hidden animate-in fade-in zoom-in duration-200">
        <div className="px-6 py-4 border-b border-gray-800 flex justify-between items-center bg-bg-root/50">
          <div className="flex items-center space-x-2">
            <Key className="h-4 w-4 text-accent" />
            <h2 className="text-sm font-bold text-white uppercase tracking-wider">View Credential</h2>
          </div>
          <button onClick={onClose} className="text-gray-500 hover:text-white transition-colors">
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="p-6 space-y-4">
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">Name</label>
            <p className="text-white font-medium">{credential.name}</p>
          </div>

          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">Type</label>
            <p className="text-white font-mono uppercase text-sm">{credential.credential_type}</p>
          </div>

          {credential.host && (
            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">Host</label>
              <p className="text-white font-mono text-sm">
                {credential.host}{credential.port ? `:${credential.port}` : ''}
              </p>
            </div>
          )}

          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">Username</label>
            <div className="flex items-center justify-between bg-bg-root border border-gray-700 rounded-lg px-4 py-2">
              <p className="text-white font-mono text-sm">{credential.username}</p>
              <button
                onClick={() => copyToClipboard(credential.username)}
                className="text-gray-400 hover:text-accent transition-colors text-xs"
              >
                {copied ? 'Copied!' : 'Copy'}
              </button>
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">Password</label>
            <div className="flex items-center justify-between bg-bg-root border border-gray-700 rounded-lg px-4 py-2">
              <p className="text-white font-mono text-sm flex-1 truncate">
                {showPassword ? credential.password : '••••••••••••'}
              </p>
              <div className="flex items-center space-x-2">
                <button
                  onClick={() => setShowPassword(!showPassword)}
                  className="text-gray-400 hover:text-accent transition-colors"
                >
                  {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
                <button
                  onClick={() => copyToClipboard(credential.password)}
                  className="text-gray-400 hover:text-accent transition-colors text-xs"
                >
                  {copied ? 'Copied!' : 'Copy'}
                </button>
              </div>
            </div>
          </div>

          <button
            onClick={onClose}
            className="w-full bg-bg-root border border-gray-700 hover:border-accent text-white py-2.5 rounded-lg text-sm font-medium transition-all"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};

export default CredentialManager;
