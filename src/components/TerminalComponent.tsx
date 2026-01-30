import React, { useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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
    const connectedRef = useRef(false);

    useEffect(() => {
        if (!terminalRef.current) return;
        if (connectedRef.current) return;

        // Initialize xterm
        const term = new Terminal({
            cursorBlink: true,
            theme: {
                background: '#09090b', // Zinc-950
                foreground: '#f4f4f5', // Zinc-100
                cursor: '#e4e4e7',
                selectionBackground: 'rgba(255, 255, 255, 0.3)',
            },
            fontFamily: 'Menlo, Monaco, "Courier New", monospace',
            fontSize: 14,
            allowProposedApi: true,
        });

        const fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        
        term.open(terminalRef.current);
        fitAddon.fit();

        term.write(`\x1b[1;34mTitan Terminal\x1b[0m\r\nConnecting to ${host}...\r\n`);

        xtermRef.current = term;
        fitAddonRef.current = fitAddon;
        connectedRef.current = true;

        let unlisten: (() => void) | undefined;

        const connect = async () => {
            try {
                // Listen for incoming data FIRST
                unlisten = await listen<string>(`ssh_data_${sessionId}`, (event) => {
                    term.write(event.payload);
                });

                // Listen for disconnect
                await listen(`ssh_closed_${sessionId}`, () => {
                    term.write('\r\n\x1b[1;31mConnection closed.\x1b[0m\r\n');
                });

                // Start connection
                await invoke('connect_ssh', { 
                    id: sessionId,
                    host,
                    port,
                    user: username,
                    password: password || undefined 
                });
                
                term.write('\x1b[1;32mConnected.\x1b[0m\r\n');

            } catch (err) {
                term.write(`\r\n\x1b[1;31mConnection failed: ${err}\x1b[0m\r\n`);
            }
        };

        connect();

        // Handle user input
        const onDataDisposable = term.onData((data) => {
            invoke('write_ssh', { id: sessionId, data }).catch(console.error);
        });

        // Handle resize
        const resizeObserver = new ResizeObserver(() => {
            fitAddon.fit();
            // Optional: send resize to backend
            // invoke('resize_ssh', { id: sessionId, rows: term.rows, cols: term.cols }).catch(console.error);
        });
        resizeObserver.observe(terminalRef.current);

        return () => {
            resizeObserver.disconnect();
            onDataDisposable.dispose();
            if (unlisten) unlisten();
            invoke('disconnect_ssh', { id: sessionId }).catch(console.error);
            term.dispose();
        };
    }, []); // Run once on mount

    return (
        <div 
            ref={terminalRef} 
            className={`w-full h-full overflow-hidden ${className || ''}`}
            data-testid="terminal-container"
        />
    );
};

export default TerminalComponent;