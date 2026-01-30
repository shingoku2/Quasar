import React, { useEffect, useRef, useState } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import SessionToolbar from './SessionToolbar';
import '@xterm/xterm/css/xterm.css';

interface TerminalComponentProps {
    className?: string;
    sessionId: string;
    host: string;
    port?: number;
    username: string;
    password?: string; // Optional for now, might prompt later
}

const TerminalComponent: React.FC<TerminalComponentProps> = ({ 
    className, sessionId, host, port = 22, username, password 
}) => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const xtermRef = useRef<Terminal | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const [clipboardSync, setClipboardSync] = useState(false);
    const [latency, setLatency] = useState<number | undefined>(undefined);
    const [bandwidth, setBandwidth] = useState<string | undefined>(undefined);

    useEffect(() => {
        if (!terminalRef.current) return;
        
        // Strict Mode protection
        if (xtermRef.current) return;

        console.log("Terminal mounting", sessionId);

        const term = new Terminal({
            cursorBlink: true,
            theme: {
                background: '#000000',
                foreground: '#ffffff',
            },
            fontFamily: 'monospace',
            fontSize: 14,
        });

        const fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        
        term.open(terminalRef.current);
        term.write('Terminal Initialized.\r\n');
        
        xtermRef.current = term;
        fitAddonRef.current = fitAddon;

        // Fit after a small delay
        setTimeout(() => {
            try {
                fitAddon.fit();
                term.write('Dimensions set.\r\n');
            } catch (e) {
                term.write(`Fit error: ${e}\r\n`);
            }
        }, 100);

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
                    password: password || undefined 
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
            console.log("Terminal unmounting", sessionId);
            isMounted = false;
            resizeObserver.disconnect();
            onDataDisposable.dispose();
            if (unlistenData) unlistenData();
            if (unlistenClosed) unlistenClosed();
            if (unlistenStats) unlistenStats();
            
            invoke('disconnect_ssh', { id: sessionId }).catch(e => {
                console.log("Disconnect result:", e);
            });
            
            term.dispose();
            xtermRef.current = null;
        };
    }, [sessionId, host, port, username, password]);

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