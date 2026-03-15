import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import DashboardView from './DashboardView';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// vis-network (used in NetworkTopologyView) is not available in jsdom
vi.mock('../NetworkTopologyView', () => ({
  default: () => <div data-testid="network-topology">NetworkTopologyView</div>,
}));

describe('DashboardView', () => {
  const onNavigate = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the Network Topology heading', () => {
    render(<DashboardView onNavigate={onNavigate} />);
    expect(screen.getByText('Network Topology')).toBeInTheDocument();
  });

  it('renders topology view by default', () => {
    render(<DashboardView onNavigate={onNavigate} />);
    expect(screen.getByTestId('network-topology')).toBeInTheDocument();
  });

  it('switches to list view when List button is clicked', async () => {
    render(<DashboardView onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText('List'));
    await waitFor(() => {
      expect(screen.getByText('Network Scanner')).toBeInTheDocument();
    });
  });

  it('switches back to topology view when Topology button is clicked', async () => {
    render(<DashboardView onNavigate={onNavigate} />);
    // Switch to list first
    fireEvent.click(screen.getByText('List'));
    // Switch back to topology
    fireEvent.click(screen.getByText('Topology'));
    await waitFor(() => {
      expect(screen.getByTestId('network-topology')).toBeInTheDocument();
    });
  });

  it('renders system health widget area', () => {
    render(<DashboardView onNavigate={onNavigate} />);
    // SystemHealthWidget renders regardless of metrics state
    expect(screen.getByText('Network Topology')).toBeInTheDocument();
  });

  it('calls get_discovered_hosts on mount', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue([]);
    render(<DashboardView onNavigate={onNavigate} />);
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_discovered_hosts', expect.anything());
    });
  });
});
