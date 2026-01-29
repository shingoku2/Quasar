import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import App from './App';
import '@testing-library/jest-dom';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve('Hello, World! You\'ve been greeted from Rust!'))
}));

describe('App', () => {
  it('renders welcome message', () => {
    render(<App />);
    expect(screen.getByText('Welcome to Tauri + React')).toBeInTheDocument();
  });

  it('greets when button is clicked', async () => {
    render(<App />);
    const input = screen.getByPlaceholderText('Enter a name...');
    const button = screen.getByRole('button', { name: /Greet/i });

    fireEvent.change(input, { target: { value: 'World' } });
    fireEvent.click(button);

    expect(await screen.findByText("Hello, World! You've been greeted from Rust!")).toBeInTheDocument();
  });
});
