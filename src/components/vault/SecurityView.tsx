import React, { useState } from 'react';
import { Key, Shield, Settings, FileText } from 'lucide-react';
import CredentialManager from './CredentialManager';
import KnownHostsManager from './KnownHostsManager';
import VaultSettings from './VaultSettings';
import AuditLogViewer from './AuditLogViewer';

type SecurityTab = 'credentials' | 'known-hosts' | 'settings' | 'audit-log';

const SecurityView: React.FC = () => {
  const [activeTab, setActiveTab] = useState<SecurityTab>('credentials');

  const renderContent = () => {
    switch (activeTab) {
      case 'credentials':
        return <CredentialManager />;
      case 'known-hosts':
        return <KnownHostsManager />;
      case 'settings':
        return <VaultSettings />;
      case 'audit-log':
        return <AuditLogViewer />;
      default:
        return <CredentialManager />;
    }
  };

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="border-b border-gray-800 bg-bg-sidebar">
        <div className="flex space-x-1 px-6 pt-4">
          <button
            onClick={() => setActiveTab('credentials')}
            className={`flex items-center space-x-2 px-4 py-3 rounded-t-lg font-medium text-sm transition-all ${
              activeTab === 'credentials'
                ? 'bg-bg-root text-accent border-t border-l border-r border-gray-700'
                : 'text-gray-400 hover:text-gray-300 hover:bg-bg-root/50'
            }`}
          >
            <Key className="h-4 w-4" />
            <span>Credentials</span>
          </button>
          <button
            onClick={() => setActiveTab('known-hosts')}
            className={`flex items-center space-x-2 px-4 py-3 rounded-t-lg font-medium text-sm transition-all ${
              activeTab === 'known-hosts'
                ? 'bg-bg-root text-accent border-t border-l border-r border-gray-700'
                : 'text-gray-400 hover:text-gray-300 hover:bg-bg-root/50'
            }`}
          >
            <Shield className="h-4 w-4" />
            <span>Known Hosts</span>
          </button>
          <button
            onClick={() => setActiveTab('settings')}
            className={`flex items-center space-x-2 px-4 py-3 rounded-t-lg font-medium text-sm transition-all ${
              activeTab === 'settings'
                ? 'bg-bg-root text-accent border-t border-l border-r border-gray-700'
                : 'text-gray-400 hover:text-gray-300 hover:bg-bg-root/50'
            }`}
          >
            <Settings className="h-4 w-4" />
            <span>Settings</span>
          </button>
          <button
            onClick={() => setActiveTab('audit-log')}
            className={`flex items-center space-x-2 px-4 py-3 rounded-t-lg font-medium text-sm transition-all ${
              activeTab === 'audit-log'
                ? 'bg-bg-root text-accent border-t border-l border-r border-gray-700'
                : 'text-gray-400 hover:text-gray-300 hover:bg-bg-root/50'
            }`}
          >
            <FileText className="h-4 w-4" />
            <span>Audit Log</span>
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-hidden">
        {renderContent()}
      </div>
    </div>
  );
};

export default SecurityView;
