import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import AIAssistant from './AIAssistant';
import '@testing-library/jest-dom';

// Mock Tauri invoke and event
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd) => {
    if (cmd === 'check_ai_status') return Promise.resolve(true);
    if (cmd === 'list_ai_models') return Promise.resolve(['llama3', 'mistral']);
    if (cmd === 'send_ai_chat') return Promise.resolve();
    return Promise.resolve(null);
  }),
}));

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
});
