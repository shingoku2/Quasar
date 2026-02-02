import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import VaultInitDialog from './VaultInitDialog';
import VaultUnlockDialog from './VaultUnlockDialog';

interface VaultContextType {
  isVaultLocked: boolean;
  lockVault: () => Promise<void>;
  unlockVault: () => void;
}

const VaultContext = createContext<VaultContextType | undefined>(undefined);

export const useVault = () => {
  const context = useContext(VaultContext);
  if (!context) {
    throw new Error('useVault must be used within VaultProvider');
  }
  return context;
};

interface VaultProviderProps {
  children: ReactNode;
}

export const VaultProvider: React.FC<VaultProviderProps> = ({ children }) => {
  const [isInitialized, setIsInitialized] = useState<boolean | null>(null);
  const [isVaultLocked, setIsVaultLocked] = useState(true);
  const [showInitDialog, setShowInitDialog] = useState(false);
  const [showUnlockDialog, setShowUnlockDialog] = useState(false);

  const checkVaultStatus = async () => {
    try {
      const initialized = await invoke<boolean>('is_vault_initialized');
      setIsInitialized(initialized);
      
      if (initialized) {
        const locked = await invoke<boolean>('is_vault_locked');
        setIsVaultLocked(locked);
        if (locked) {
          setShowUnlockDialog(true);
        }
      } else {
        setShowInitDialog(true);
      }
    } catch (error) {
      console.error('Failed to check vault status:', error);
    }
  };

  useEffect(() => {
    checkVaultStatus();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    
    const setupListener = async () => {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten = await listen('vault-auto-locked', () => {
        setIsVaultLocked(true);
        setShowUnlockDialog(true);
      });
    };
    
    setupListener();
    
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const handleInitialized = async () => {
    setShowInitDialog(false);
    setIsInitialized(true);
    setIsVaultLocked(false);
  };

  const handleUnlocked = () => {
    setShowUnlockDialog(false);
    setIsVaultLocked(false);
  };

  const lockVault = async () => {
    try {
      await invoke('lock_vault');
      setIsVaultLocked(true);
      setShowUnlockDialog(true);
    } catch (error) {
      console.error('Failed to lock vault:', error);
    }
  };

  const unlockVault = () => {
    setShowUnlockDialog(true);
  };

  if (isInitialized === null) {
    return (
      <div className="flex items-center justify-center h-screen bg-bg-root">
        <div className="text-center">
          <div className="inline-block animate-spin rounded-full h-12 w-12 border-b-2 border-accent mb-4"></div>
          <p className="text-gray-400">Initializing security vault...</p>
        </div>
      </div>
    );
  }

  return (
    <VaultContext.Provider value={{ isVaultLocked, lockVault, unlockVault }}>
      {children}
      
      {showInitDialog && (
        <VaultInitDialog onInitialized={handleInitialized} />
      )}
      
      {showUnlockDialog && (
        <VaultUnlockDialog 
          onUnlocked={handleUnlocked}
          onCancel={isInitialized ? () => setShowUnlockDialog(false) : undefined}
        />
      )}
    </VaultContext.Provider>
  );
};
