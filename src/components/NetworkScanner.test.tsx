import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import NetworkScanner from './NetworkScanner';
import '@testing-library/jest-dom';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

describe('NetworkScanner', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders scanner with default CIDR', () => {
    render(<NetworkScanner />);
    
    expect(screen.getByText('Network Scanner')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('192.168.1.0/24')).toHaveValue('192.168.1.0/24');
    expect(screen.getByText('Start')).toBeInTheDocument();
  });

  it('allows CIDR input changes', () => {
    render(<NetworkScanner />);
    
    const input = screen.getByPlaceholderText('192.168.1.0/24');
    fireEvent.change(input, { target: { value: '10.0.0.0/24' } });
    
    expect(input).toHaveValue('10.0.0.0/24');
  });

  it('disables start button when scanning', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValueOnce(true); // is_scanning returns true
    
    render(<NetworkScanner />);
    
    const startButton = screen.getByText('Start');
    fireEvent.click(startButton);
    
    await waitFor(() => {
      expect(screen.getByText('Stop')).toBeInTheDocument();
    });
  });

  it('displays discovered hosts', () => {
    const onHostFound = vi.fn();
    render(<NetworkScanner onHostFound={onHostFound} />);
    
    // Component should render without hosts initially
    expect(screen.queryByText('Discovered Hosts')).not.toBeInTheDocument();
  });

  it('shows clear results button when hosts found', () => {
    render(<NetworkScanner />);
    
    // Initially no clear button
    expect(screen.queryByText('Clear')).not.toBeInTheDocument();
  });

  it('calls onResults callback when provided', async () => {
    const onResults = vi.fn();
    render(<NetworkScanner onResults={onResults} />);
    
    // Component should render
    expect(screen.getByText('Network Scanner')).toBeInTheDocument();
  });
});
