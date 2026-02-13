import React from 'react';
import { 
  LayoutDashboard, 
  Terminal, 
  Activity, 
  Bot, 
  Settings,
  Shield,
  Lock,
  LockOpen
} from 'lucide-react';
import { cn } from '../lib/utils';
import { useVault } from './vault/VaultProvider';
import quasarLogo from '../assets/quasar-logo.svg';

export type ViewId = 'dashboard' | 'remote' | 'monitoring' | 'ai' | 'security' | 'settings';

interface SidebarProps {
  activeView: ViewId;
  onViewChange: (view: ViewId) => void;
}

const navItems = [
  { id: 'dashboard', icon: LayoutDashboard, label: 'Dashboard' },
  { id: 'remote', icon: Terminal, label: 'Remote' },
  { id: 'monitoring', icon: Activity, label: 'Monitoring' },
  { id: 'ai', icon: Bot, label: 'AI Assistant' },
  { id: 'security', icon: Shield, label: 'Security' },
  { id: 'settings', icon: Settings, label: 'Settings' },
] as const;

const Sidebar: React.FC<SidebarProps> = ({ activeView, onViewChange }) => {
  const { isVaultLocked } = useVault();

  return (
    <aside className="w-16 lg:w-64 bg-bg-sidebar border-r border-border flex flex-col h-full transition-all duration-300">
      <div className="p-4 lg:p-5 flex flex-col items-center border-b border-border mb-2">
        <img src={quasarLogo} alt="Quasar" className="h-12 w-12 lg:h-16 lg:w-16" />
        <span className="font-bold text-base tracking-widest hidden lg:block text-white mt-2">QUASAR</span>
      </div>

      <nav className="flex-1 px-2 space-y-1 py-4 overflow-y-auto no-scrollbar">
        {navItems.map((item) => {
          const isActive = activeView === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onViewChange(item.id as ViewId)}
              className={cn(
                "w-full flex items-center rounded-lg px-3 py-2.5 transition-all group relative",
                isActive 
                  ? "bg-accent text-white font-medium shadow-lg shadow-accent/20" 
                  : "text-gray-400 hover:bg-white/5 hover:text-gray-200"
              )}
              title={item.label}
            >
              <item.icon className={cn(
                "h-5 w-5 shrink-0",
                isActive ? "text-white" : "text-gray-400 group-hover:text-gray-200"
              )} />
              <span className="ml-3 text-sm hidden lg:block truncate">{item.label}</span>
              
              {isActive && (
                <div className="absolute left-0 top-2 bottom-2 w-1 bg-accent rounded-r-full lg:hidden" />
              )}
            </button>
          );
        })}
      </nav>

      <div className="p-3 border-t border-border hidden lg:block">
        <div className={cn(
          "flex items-center space-x-2 px-3 py-2 rounded-lg text-xs font-medium",
          isVaultLocked 
            ? "bg-alert/10 text-alert" 
            : "bg-success/10 text-success"
        )}>
          {isVaultLocked ? (
            <Lock className="h-4 w-4 shrink-0" />
          ) : (
            <LockOpen className="h-4 w-4 shrink-0" />
          )}
          <span>Vault: {isVaultLocked ? 'Locked' : 'Unlocked'}</span>
        </div>
      </div>
    </aside>
  );
};

export default Sidebar;
