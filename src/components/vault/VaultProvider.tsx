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
  const [initError, setInitError] = useState<string | null>(null);

  const getErrorMessage = (error: unknown): string => {
    if (typeof error === 'string' && error.trim().length > 0) {
      return error;
    }

    if (error instanceof Error && error.message.trim().length > 0) {
      return error.message;
    }

    if (error && typeof error === 'object' && 'message' in error) {
      const message = (error as { message?: unknown }).message;
      if (typeof message === 'string' && message.trim().length > 0) {
        return message;
      }
    }

    return 'Failed to initialize security vault';
  };

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
      setInitError(getErrorMessage(error));
      setIsInitialized(null);
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
    if (initError) {
      return (
        <div className="flex items-center justify-center h-screen bg-bg-root">
          <div className="text-center max-w-md">
            <div className="bg-alert/10 border border-alert/30 rounded-lg p-6 mb-4">
              <h2 className="text-alert text-lg font-bold mb-2">Vault Initialization Error</h2>
              <p className="text-gray-300 text-sm">{initError}</p>
            </div>
            <button
              onClick={() => {
                setInitError(null);
                setIsInitialized(null);
                checkVaultStatus();
              }}
              className="bg-accent hover:bg-accent/80 text-white px-6 py-2 rounded-lg text-sm font-bold transition-all"
            >
              Retry
            </button>
          </div>
        </div>
      );
    }
    
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
