import React, { useState } from 'react';
import { KeyRound, ShieldQuestion, X } from 'lucide-react';

interface CredentialPromptProps {
  hostName: string;
  username: string;
  onSubmit: (password: string) => void;
  onCancel: () => void;
}

const CredentialPrompt: React.FC<CredentialPromptProps> = ({ 
  hostName, username, onSubmit, onCancel 
}) => {
  const [password, setPassword] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (password) {
      onSubmit(password);
    }
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-md">
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-sm overflow-hidden animate-in fade-in zoom-in duration-200">
        <div className="px-6 py-4 border-b border-gray-800 flex justify-between items-center bg-bg-root/50">
          <div className="flex items-center space-x-2">
            <KeyRound className="h-4 w-4 text-accent" />
            <h2 className="text-sm font-bold text-white uppercase tracking-wider">Authentication Required</h2>
          </div>
          <button onClick={onCancel} className="text-gray-500 hover:text-white transition-colors">
            <X className="h-4 w-4" />
          </button>
        </div>
        
        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          <div className="text-center space-y-1">
            <div className="inline-flex p-3 bg-accent/10 rounded-full mb-2">
              <ShieldQuestion className="h-6 w-6 text-accent" />
            </div>
            <p className="text-gray-300 text-sm">
              Please enter the password for <span className="text-white font-bold">{username}</span> on <span className="text-white font-bold">{hostName}</span>
            </p>
          </div>

          <div>
            <input 
              autoFocus
              type="password" 
              className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2.5 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all placeholder:text-gray-600"
              placeholder="Password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
          </div>

          <div className="pt-2 flex flex-col space-y-2">
            <button 
              type="submit"
              className="w-full bg-accent hover:bg-accent/80 text-white py-2.5 rounded-lg text-sm font-bold transition-all shadow-lg shadow-accent/20"
            >
              Connect Session
            </button>
            <button 
              type="button"
              onClick={onCancel}
              className="w-full py-2.5 text-xs font-medium text-gray-500 hover:text-gray-300 transition-colors"
            >
              Cancel
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export default CredentialPrompt;
