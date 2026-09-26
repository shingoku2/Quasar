import React from 'react';
import { Edit2, Eye, Server, Trash2 } from 'lucide-react';
import type { CredentialSummary } from './types';

const TYPE_COLORS: Record<string, string> = {
  ssh: 'text-blue-400',
  rdp: 'text-purple-400',
  database: 'text-green-400',
  api: 'text-yellow-400',
  other: 'text-gray-400',
};

const CredentialCard: React.FC<{
  credential: CredentialSummary;
  onView: () => void;
  onEdit: () => void;
  onDelete: () => void;
}> = ({ credential, onView, onEdit, onDelete }) => (
  <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-4 hover:border-accent/50 transition-all">
    <div className="flex justify-between items-start mb-3">
      <div className="flex-1 min-w-0">
        <h3 className="text-white font-bold truncate">{credential.name}</h3>
        <p className="text-sm text-gray-500 truncate">{credential.username}</p>
      </div>
      <span className={`text-xs font-mono uppercase px-2 py-1 rounded ${TYPE_COLORS[credential.credential_type] || TYPE_COLORS.other}`}>
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
        aria-label="View credential"
        className="text-gray-400 hover:text-accent transition-colors p-1"
        title="View"
      >
        <Eye className="h-4 w-4" />
      </button>
      <button
        onClick={onEdit}
        aria-label="Edit credential"
        className="text-gray-400 hover:text-accent transition-colors p-1"
        title="Edit"
      >
        <Edit2 className="h-4 w-4" />
      </button>
      <button
        onClick={onDelete}
        aria-label="Delete credential"
        className="text-gray-400 hover:text-alert transition-colors p-1"
        title="Delete"
      >
        <Trash2 className="h-4 w-4" />
      </button>
    </div>
  </div>
);

export default CredentialCard;
