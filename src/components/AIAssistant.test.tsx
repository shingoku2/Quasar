import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import AIAssistant from './AIAssistant';
import '@testing-library/jest-dom';

// Mock Tauri invoke and event
const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn((cmd: string, _args?: unknown) => {
    if (cmd === 'check_ai_status') return Promise.resolve(true);
    if (cmd === 'list_ai_models') return Promise.resolve(['llama3', 'mistral']);
    if (cmd === 'get_saved_hosts') {
      return Promise.resolve([
        { id: 'host-1', name: 'Prod Server', address: '10.0.0.1', protocol: 'ssh', port: 22 },
      ]);
    }
    if (cmd === 'get_discovered_hosts') return Promise.resolve([]);
    if (cmd === 'is_scanning') return Promise.resolve(false);
    if (cmd === 'get_scan_progress') return Promise.resolve(null);
    if (cmd === 'send_ai_chat') return Promise.resolve();
    return Promise.resolve(null);
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event, callback) => {
    if (event === 'ai-chat-response') {
      // Simulate response immediately for test
      callback({ payload: { content: 'Hello!', done: false } });
      setTimeout(() => callback({ payload: { content: '', done: true } }), 10);
    }
    return Promise.resolve(() => {});
  }),
}));

// Mock scrollIntoView
window.HTMLElement.prototype.scrollIntoView = vi.fn();

describe('AIAssistant Component', () => {
  it('renders status and model selector', async () => {
    render(<AIAssistant />);
    
    await waitFor(() => {
      expect(screen.getByText('Ollama Online')).toBeInTheDocument();
      expect(screen.getByDisplayValue('llama3')).toBeInTheDocument();
    });
  });

  it('sends message and displays response', async () => {
    render(<AIAssistant />);
    
    // Wait for load
    await waitFor(() => expect(screen.getByText('Ollama Online')).toBeInTheDocument());

    const input = screen.getByPlaceholderText('Ask Quasar AI...');
    fireEvent.change(input, { target: { value: 'Hi' } });
    
    const sendButton = screen.getByText('Send');
    fireEvent.click(sendButton);

    await waitFor(() => {
      expect(screen.getByText('Hi')).toBeInTheDocument(); // User message
      expect(screen.getByText('Hello!')).toBeInTheDocument(); // AI response
    });
  });
  it('includes saved hosts in the Rust-provided network context', async () => {
    mockInvoke.mockClear();
    render(<AIAssistant />);

    await waitFor(() => expect(screen.getByText('Ollama Online')).toBeInTheDocument());
    fireEvent.change(screen.getByPlaceholderText('Ask Quasar AI...'), { target: { value: 'Status?' } });
    fireEvent.click(screen.getByText('Send'));

    await waitFor(() => {
      const sendCall = mockInvoke.mock.calls.find(([command]) => command === 'send_ai_chat');
      expect(sendCall).toBeDefined();
      expect(sendCall?.[1]).toEqual(expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({ role: 'system', content: expect.stringContaining('Prod Server') }),
        ]),
      }));
    });
  });
});
