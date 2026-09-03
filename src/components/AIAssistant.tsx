import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface Message {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

interface ChatResponse {
  content: string;
  done: boolean;
}

interface ScanProgress {
  total: number;
  completed: number;
  current_ip?: string | null;
}

interface ServiceInfo {
  port: number;
  protocol: string;
  service: string;
  version?: string | null;
}

interface DiscoveredHost {
  ip: string;
  hostname?: string | null;
  device_type: string;
  vendor?: string | null;
  scan_count: number;
  services: ServiceInfo[];
}

interface SavedHost {
  id: string;
  name: string;
  address: string;
  protocol: string;
  port?: number | null;
}

const AIAssistant: React.FC = () => {
  const [status, setStatus] = useState<'checking' | 'connected' | 'disconnected'>('checking');
  const [models, setModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>('');
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [generating, setGenerating] = useState(false);
  const isSending = useRef(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const unlistenRef = useRef<() => void | undefined>(undefined);

  useEffect(() => {
    checkStatus();
    return () => {
      if (unlistenRef.current) unlistenRef.current();
    };
  }, []);

  const buildNetworkContextMessage = async (): Promise<Message> => {
    try {
      const [isScanningResult, progressResult, discoveredHostsResult, savedHostsResult] = await Promise.allSettled([
        invoke<boolean>('is_scanning'),
        invoke<ScanProgress>('get_scan_progress'),
        invoke<DiscoveredHost[]>('get_discovered_hosts', { limit: 50 }),
        invoke<SavedHost[]>('get_saved_hosts'),
      ]);

      const isScanning = isScanningResult.status === 'fulfilled' ? isScanningResult.value : false;
      const progress = progressResult.status === 'fulfilled' ? progressResult.value : undefined;
      const discoveredHosts = discoveredHostsResult.status === 'fulfilled' ? discoveredHostsResult.value : [];
      const savedHosts = savedHostsResult.status === 'fulfilled' ? savedHostsResult.value : [];

      const safeHosts = Array.isArray(discoveredHosts) ? discoveredHosts : [];
      const hostSummary = safeHosts.slice(0, 10).map((host) => ({
        ip: host.ip,
        hostname: host.hostname ?? null,
        deviceType: host.device_type,
        vendor: host.vendor ?? null,
        scanCount: host.scan_count,
        services: (host.services ?? []).slice(0, 6).map((service) => `${service.service}:${service.port}/${service.protocol}`),
      }));

      const safeSavedHosts = Array.isArray(savedHosts) ? savedHosts : [];
      const savedSummary = safeSavedHosts.slice(0, 15).map((host) => ({
        name: host.name,
        address: host.address,
        protocol: host.protocol,
        port: host.port ?? null,
      }));

      const discoveredAddressSet = new Set(safeHosts.map((host) => host.ip));
      const overlapCount = safeSavedHosts.filter((host) => discoveredAddressSet.has(host.address)).length;

      return {
        role: 'system',
        content: [
          'Quasar network context (auto-generated):',
          `- Scan status: ${isScanning ? 'running' : 'idle'}`,
          `- Scan progress: ${progress?.completed ?? 0}/${progress?.total ?? 0} scanned, ${safeHosts.length} alive`,
          `- Discovered hosts in database: ${safeHosts.length}`,
          `- Saved remote hosts: ${safeSavedHosts.length}`,
          `- Saved/discovered address overlap: ${overlapCount}`,
          `- Top discovered hosts snapshot: ${JSON.stringify(hostSummary)}`,
          `- Top saved hosts snapshot: ${JSON.stringify(savedSummary)}`,
          'Use this context when troubleshooting network setup, connectivity, ports, and host-level issues.',
        ].join('\n'),
      };
    } catch {
      return {
        role: 'system',
        content: 'Quasar network context is currently unavailable. Continue responding normally and ask the user to run a network scan if needed.',
      };
    }
  };

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const checkStatus = async () => {
    try {
      const isConnected = await invoke<boolean>('check_ai_status');
      if (isConnected) {
        setStatus('connected');
        const availableModels = await invoke<string[]>('list_ai_models');
        setModels(availableModels);
        if (availableModels.length > 0) {
          setSelectedModel(availableModels[0]);
        }
      } else {
        setStatus('disconnected');
      }
    } catch (err) {
      console.error('Failed to check AI status:', err);
      setStatus('disconnected');
    }
  };

  const handleSend = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isSending.current || !selectedModel) return;

    isSending.current = true;
    setGenerating(true);
    
    const userMessage: Message = { role: 'user', content: input };
    setMessages(prev => [...prev, userMessage]);
    setInput('');

    // Add placeholder for assistant response
    setMessages(prev => [...prev, { role: 'assistant', content: '' }]);

    try {
      // Cleanup previous listener if any (safety)
      if (unlistenRef.current) unlistenRef.current();

      // Set up listener for stream
      const unlisten = await listen<ChatResponse>('ai-chat-response', (event) => {
        if (event.payload.done) {
          setGenerating(false);
          isSending.current = false;
          unlisten();
          unlistenRef.current = undefined;
        } else {
          setMessages(prev => {
            const newMessages = [...prev];
            const lastIdx = newMessages.length - 1;
            const lastMessage = { ...newMessages[lastIdx] };
            if (lastMessage.role === 'assistant') {
              lastMessage.content += event.payload.content;
              newMessages[lastIdx] = lastMessage;
            }
            return newMessages;
          });
        }
      });
      unlistenRef.current = unlisten;

      const networkContext = await buildNetworkContextMessage();

      await invoke('send_ai_chat', {
        model: selectedModel,
        messages: [networkContext, ...messages, userMessage].map(m => ({ role: m.role, content: m.content })),
      });
    } catch (err) {
      console.error('Failed to send chat:', err);
      setMessages(prev => [...prev, { role: 'system', content: `Error: ${err}` }]);
      setGenerating(false);
      isSending.current = false;
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = undefined;
      }
    }
  };

  return (
    <div className="flex flex-col h-full bg-gray-900 text-white">
      {/* Header */}
      <div className="p-4 border-b border-gray-800 flex justify-between items-center">
        <h2 className="text-lg font-bold">Quasar AI Assistant</h2>
        <div className="flex items-center space-x-4">
          <div className="flex items-center space-x-2">
            <span className={`h-2 w-2 rounded-full ${status === 'connected' ? 'bg-green-500' : 'bg-red-500'}`}></span>
            <span className="text-xs text-gray-400 uppercase">{status === 'connected' ? 'Ollama Online' : 'Ollama Offline'}</span>
          </div>
          {status === 'connected' && (
            <select
              value={selectedModel}
              onChange={(e) => setSelectedModel(e.target.value)}
              className="bg-gray-800 border border-gray-700 text-xs rounded px-2 py-1 focus:outline-none focus:border-blue-500"
            >
              {models.map(m => <option key={m} value={m}>{m}</option>)}
            </select>
          )}
        </div>
      </div>

      {/* Messages Area */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {status === 'disconnected' && (
          <div className="p-4 bg-red-900/20 border border-red-900 rounded text-red-200 text-sm">
            <p className="font-bold">Ollama is not detected.</p>
            <p className="mt-1">Please ensure Ollama is installed and running on your local machine.</p>
            <a href="https://ollama.com" target="_blank" rel="noreferrer" className="underline mt-2 block">Download Ollama</a>
            <button onClick={checkStatus} className="mt-3 bg-red-800 hover:bg-red-700 px-3 py-1 rounded text-xs">Retry Connection</button>
          </div>
        )}
        
        {messages.map((msg, idx) => (
          <div key={idx} className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
            <div className={`max-w-[80%] rounded-lg p-3 text-sm ${
              msg.role === 'user' ? 'bg-blue-600 text-white' : 
              msg.role === 'system' ? 'bg-red-900/50 text-red-200 border border-red-800' :
              'bg-gray-800 text-gray-200'
            }`}>
              {msg.role === 'assistant' && msg.content === '' && generating && idx === messages.length - 1 ? (
                <span className="animate-pulse">Thinking...</span>
              ) : (
                <div className="whitespace-pre-wrap">{msg.content}</div>
              )}
            </div>
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>

      {/* Input Area */}
      <form onSubmit={handleSend} className="p-4 border-t border-gray-800 bg-gray-900">
        <div className="flex space-x-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            disabled={status !== 'connected' || generating}
            placeholder={status === 'connected' ? "Ask Quasar AI..." : "AI Unavailable"}
            className="flex-1 bg-gray-800 border border-gray-700 rounded px-4 py-2 text-sm focus:outline-none focus:border-blue-500 disabled:opacity-50"
          />
          <button
            type="submit"
            disabled={status !== 'connected' || generating || !input.trim()}
            className="bg-blue-600 hover:bg-blue-500 disabled:bg-gray-700 disabled:text-gray-500 text-white px-4 py-2 rounded font-medium transition-colors"
          >
            Send
          </button>
        </div>
      </form>
    </div>
  );
};

export default AIAssistant;