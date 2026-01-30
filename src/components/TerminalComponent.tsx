import React, { useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';

interface TerminalComponentProps {
    className?: string;
}

const TerminalComponent: React.FC<TerminalComponentProps> = ({ className }) => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const xtermRef = useRef<Terminal | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);

    useEffect(() => {
        if (!terminalRef.current) return;

        // Initialize xterm
        const term = new Terminal({
            cursorBlink: true,
            theme: {
                background: '#09090b', // Zinc-950 (matches likely dark mode)
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

        // Write a welcome message for now
        term.write('\x1b[1;34mTitan Terminal\x1b[0m\r\nInitializing...\r\n');

        xtermRef.current = term;
        fitAddonRef.current = fitAddon;

        // Handle resize
        const resizeObserver = new ResizeObserver(() => {
            fitAddon.fit();
        });
        resizeObserver.observe(terminalRef.current);

        return () => {
            resizeObserver.disconnect();
            term.dispose();
        };
    }, []);

    return (
        <div 
            ref={terminalRef} 
            className={`w-full h-full overflow-hidden ${className || ''}`}
            data-testid="terminal-container"
        />
    );
};

export default TerminalComponent;
