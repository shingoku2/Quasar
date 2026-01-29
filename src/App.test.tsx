import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import App from './App';
import '@testing-library/jest-dom';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve('Hello, World! You\'ve been greeted from Rust!'))
}));

describe('App', () => {
  it('renders layout with dashboard', () => {
    render(<App />);
    expect(screen.getByText('Welcome to Project Titan')).toBeInTheDocument();
  });
});
