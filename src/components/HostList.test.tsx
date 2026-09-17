import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import HostList from './HostList';
import '@testing-library/jest-dom';

// Mock Tauri invoke
const { mockInvoke, defaultInvoke } = vi.hoisted(() => {
  const mockHosts = [
    { id: '1', name: 'Prod Web', address: '10.0.0.10', protocol: 'ssh', port: 22 },
    { id: '2', name: 'Dev DB', address: '10.0.0.20', protocol: 'database', port: 5432 },
  ];

  const defaultInvoke = (cmd: string, _args?: unknown): Promise<unknown> => {
    if (cmd === 'get_saved_hosts') return Promise.resolve(mockHosts);
    if (cmd === 'remove_saved_hosts') return Promise.resolve();
    return Promise.resolve(null);
  };
  return { mockInvoke: vi.fn(defaultInvoke), defaultInvoke, mockHosts };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));

// Mock subcomponents
vi.mock('./Discovery', () => ({
  default: () => <div data-testid="discovery-mock" />,
}));

vi.mock('./HealthCheckBadge', () => ({
  default: ({ host }: { host: string }) => <span data-testid={`health-badge-${host}`}>OK</span>,
}));

// Mock window.confirm
vi.spyOn(window, 'confirm').mockImplementation(() => true);

describe('HostList Component', () => {
  const mockProps = {
    onConnect: vi.fn(),
    onSftp: vi.fn(),
    onAddHost: vi.fn(),
  };

  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(defaultInvoke);
    vi.clearAllMocks();
  });

  it('renders loading state initially', async () => {
    // We don't await the resolve here to catch the loading state
    render(<HostList {...mockProps} />);
    expect(screen.getByText('Loading hosts...')).toBeInTheDocument();

    // Let the fetch complete to avoid unhandled state updates
    await waitFor(() => {
      expect(screen.queryByText('Loading hosts...')).not.toBeInTheDocument();
    });
  });

  it('renders hosts list after fetching', async () => {
    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.queryByText('Loading hosts...')).not.toBeInTheDocument();
    });

    expect(screen.getByText('Prod Web')).toBeInTheDocument();
    expect(screen.getByText('Dev DB')).toBeInTheDocument();
    expect(screen.getByText('10.0.0.10:22')).toBeInTheDocument();
    expect(screen.getByText('10.0.0.20:5432')).toBeInTheDocument();
  });

  it('handles filtering of hosts', async () => {
    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('Prod Web')).toBeInTheDocument();
    });

    const filterInput = screen.getByPlaceholderText('Filter hosts...');
    fireEvent.change(filterInput, { target: { value: 'dev' } });

    expect(screen.queryByText('Prod Web')).not.toBeInTheDocument();
    expect(screen.getByText('Dev DB')).toBeInTheDocument();
  });

  it('handles missing hosts and no matches', async () => {
    // Override invoke for this test to return empty array
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('No hosts added yet.')).toBeInTheDocument();
    });

    // Reset and try filter no match
    mockInvoke.mockImplementation(defaultInvoke);

    // We already rendered above. So we need to clean up and re-render OR just unmount the previous render.
    // However, Testing Library renders in isolation per test, but multiple calls to render within one test
    // appends to the body. Better to destructure unmount from the first render.
  });

  it('handles filtering with no match', async () => {
    mockInvoke.mockImplementation(defaultInvoke);

    render(<HostList {...mockProps} />);
    await waitFor(() => {
      expect(screen.getByText('Prod Web')).toBeInTheDocument();
    });

    const filterInput = screen.getByPlaceholderText('Filter hosts...');
    fireEvent.change(filterInput, { target: { value: 'nonexistent' } });

    expect(screen.getByText('No hosts match your filter.')).toBeInTheDocument();
  });

  it('handles duplicate hosts and remove functionality', async () => {
    const duplicatesList = [
      { id: '1', name: 'Prod Web', address: '10.0.0.10', protocol: 'ssh', port: 22 },
      { id: '2', name: 'Prod Web Duplicate', address: '10.0.0.10', protocol: 'ssh', port: 22 }, // Duplicate based on logic
    ];

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') return Promise.resolve(duplicatesList);
      if (cmd === 'remove_saved_hosts') return Promise.resolve();
      return Promise.resolve(null);
    });

    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('1 duplicate detected')).toBeInTheDocument();
    });

    const removeBtn = screen.getByText('Remove duplicates (1)');
    expect(removeBtn).not.toBeDisabled();

    fireEvent.click(removeBtn);

    expect(window.confirm).toHaveBeenCalledWith('Remove 1 duplicate saved host(s)?');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('remove_saved_hosts', { ids: ['2'] });
    });
  });

  it('handles removing a single host', async () => {
    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('Prod Web')).toBeInTheDocument();
    });

    const removeButtons = screen.getAllByText('Remove');
    fireEvent.click(removeButtons[0]);

    expect(window.confirm).toHaveBeenCalledWith('Remove Prod Web from saved hosts?');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('remove_saved_hosts', { ids: ['1'] });
    });
  });

  it('calls respective action handlers', async () => {
    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('Prod Web')).toBeInTheDocument();
    });

    // SSH Host (has Connect, SFTP, Edit)
    const connectButtons = screen.getAllByText('Connect');
    fireEvent.click(connectButtons[0]); // Prod Web is SSH and connectable
    expect(mockProps.onConnect).toHaveBeenCalledWith(expect.objectContaining({ id: '1', name: 'Prod Web' }));

    const sftpButtons = screen.getAllByText('SFTP');
    fireEvent.click(sftpButtons[0]);
    expect(mockProps.onSftp).toHaveBeenCalledWith(expect.objectContaining({ id: '1' }));

    const editButtons = screen.getAllByText('Edit');
    fireEvent.click(editButtons[0]);
    expect(mockProps.onAddHost).toHaveBeenCalledWith(expect.objectContaining({ id: '1' }));
  });
});
