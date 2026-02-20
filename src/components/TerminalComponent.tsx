import React, { useEffect, useRef, useState } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import SessionToolbar from './SessionToolbar';
import '@xterm/xterm/css/xterm.css';

/** Built-in terminal themes (Phase 8: SSH feature enhancements). */
export const TERMINAL_THEMES = {
    default: { background: '#000000', foreground: '#ffffff' },
    quasar: { background: '#0f1923', foreground: '#e2e8f0' },
    solarizedDark: { background: '#002b36', foreground: '#839496' },
    solarizedLight: { background: '#fdf6e3', foreground: '#657b83' },
    monokai: { background: '#272822', foreground: '#f8f8f2' },
    nord: { background: '#2e3440', foreground: '#d8dee9' },
} as const;

export type TerminalThemeId = keyof typeof TERMINAL_THEMES;

interface TerminalComponentProps {
    className?: string;
    sessionId: string;
    host: string;
    port?: number;
    username: string;
    password?: string;
    /** When set, backend loads this credential (password or SSH key) for auth. */
    credentialId?: string;
    /** Terminal theme (default: 'default'). */
    theme?: TerminalThemeId;
    /** Terminal font family (default: 'monospace'). */
    fontFamily?: string;
    /** Terminal font size in px (default: 14). */
    fontSize?: number;
}

const TerminalComponent: React.FC<TerminalComponentProps> = ({ 
    className, sessionId, host, port = 22, username, password, credentialId,
    theme = 'default', fontFamily = 'monospace', fontSize = 14,
}) => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const xtermRef = useRef<Terminal | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const [clipboardSync, setClipboardSync] = useState(false);
    const [latency, setLatency] = useState<number | undefined>(undefined);
    const [bandwidth, setBandwidth] = useState<string | undefined>(undefined);
    const [isReady, setIsReady] = useState(false);

    // First effect: wait for container to be ready
    useEffect(() => {
        if (!terminalRef.current) {
            return;
        }
        
        const checkAndSetReady = () => {
            if (!terminalRef.current) return;
            const rect = terminalRef.current.getBoundingClientRect();
            if (rect.width > 0 && rect.height > 0) {
                setIsReady(true);
            }
        };

        // Check immediately
        checkAndSetReady();

        // Also check after a delay in case of initial render issues
        const timer = setTimeout(() => {
            checkAndSetReady();
        }, 200);

        return () => {
            clearTimeout(timer);
        };
    }, [sessionId]);

    // Second effect: initialize terminal once ready
    useEffect(() => {
        if (!isReady) {
            return;
        }
        
        if (!terminalRef.current) {
            return;
        }

        // When deps (e.g. theme, fontFamily, fontSize) change, React runs the previous
        // effect's cleanup first (dispose + xtermRef.current = null), then this body,
        // so we re-initialize with the new values. No guard here so prop changes apply.
        const themeConfig = TERMINAL_THEMES[theme] ?? TERMINAL_THEMES.default;
        const term = new Terminal({
            cursorBlink: true,
            theme: {
                background: themeConfig.background,
                foreground: themeConfig.foreground,
            },
            fontFamily,
            fontSize,
        });

        const fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        
        term.open(terminalRef.current);
        term.write('Terminal Initialized.\r\n');
        
        xtermRef.current = term;
        fitAddonRef.current = fitAddon;

        // Fit after a small delay to ensure container is fully rendered
        setTimeout(() => {
            try {
                fitAddon.fit();
                term.write('Dimensions set.\r\n');
            } catch (e) {
                console.error('Fit error:', e);
                term.write(`Fit error: ${e}\r\n`);
            }
        }, 150);

        let unlistenData: (() => void) | undefined;
        let unlistenClosed: (() => void) | undefined;
        let unlistenStats: (() => void) | undefined;
        let isMounted = true;

        const initSession = async () => {
            try {
                term.write(`Connecting to ${host} as ${username}...\r\n`);
                
                unlistenData = await listen<string>(`ssh_data_${sessionId}`, (event) => {
                    term.write(event.payload);
                });
                
                unlistenClosed = await listen(`ssh_closed_${sessionId}`, () => {
                    term.write('\r\nConnection closed.\r\n');
                });

                unlistenStats = await listen<{bandwidth: string, latency: number}>(`ssh_stats_${sessionId}`, (event) => {
                    setBandwidth(event.payload.bandwidth);
                    setLatency(event.payload.latency);
                });

                if (!isMounted) return;

                await invoke('connect_ssh', { 
                    id: sessionId,
                    host,
                    port,
                    user: username,
                    password: password || undefined,
                    credentialId: credentialId || undefined
                });
                
                if (isMounted) {
                    term.write('Session Established.\r\n');
                    // Sync initial size
                    invoke('resize_ssh', { 
                        id: sessionId, 
                        rows: term.rows, 
                        cols: term.cols 
                    }).catch(console.error);
                }

            } catch (err) {
                if (isMounted) {
                    term.write(`\r\nConnection Error: ${err}\r\n`);
                }
            }
        };

        initSession();

        const onDataDisposable = term.onData((data) => {
            invoke('write_ssh', { id: sessionId, data }).catch(e => {
                if (!JSON.stringify(e).includes("Session not found")) {
                    console.error("Write error:", e);
                }
            });
        });

        const resizeObserver = new ResizeObserver(() => {
            try {
                fitAddon.fit();
                // Sync resize with backend
                if (term.cols > 0 && term.rows > 0) {
                    invoke('resize_ssh', { 
                        id: sessionId, 
                        rows: term.rows, 
                        cols: term.cols 
                    }).catch(e => {
                        if (!JSON.stringify(e).includes("Session not found")) {
                            console.error("Resize error:", e);
                        }
                    });
                }
            } catch (e) {
                console.error("Resize observer error:", e);
            }
        });
        resizeObserver.observe(terminalRef.current);

        return () => {
            isMounted = false;
            resizeObserver.disconnect();
            onDataDisposable.dispose();
            if (unlistenData) unlistenData();
            if (unlistenClosed) unlistenClosed();
            if (unlistenStats) unlistenStats();
            
            invoke('disconnect_ssh', { id: sessionId }).catch(() => {
                // Session cleanup
            });
            
            term.dispose();
            xtermRef.current = null;
        };
    }, [isReady, sessionId, host, port, username, password, credentialId, theme, fontFamily, fontSize]);

    return (
        <div className={`flex flex-col h-full ${className || ''}`} data-testid="terminal-wrapper">
            <SessionToolbar 
                sessionId={sessionId}
                latency={latency}
                bandwidth={bandwidth}
                clipboardSync={clipboardSync}
                onToggleClipboard={() => setClipboardSync(!clipboardSync)}
            />
            <div 
                ref={terminalRef} 
                className="flex-1 bg-black border border-gray-700 overflow-hidden"
                data-testid="terminal-container"
            />
        </div>
    );
};

export default TerminalComponent;