import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import QuickConnectWidget from './QuickConnectWidget';
import '@testing-library/jest-dom';

const { mockInvoke } = vi.hoisted(() => ({ mockInvoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));

const mockHosts = [
  { id: '1', name: 'Prod Web', address: '10.0.0.1', port: 22, username: 'root', protocol: 'ssh' },
  { id: '2', name: 'Dev Box', address: '10.0.0.2', port: 3389, username: 'admin', protocol: 'rdp' },
  { id: '3', name: 'DB Server', address: '10.0.0.3', port: null, username: null, protocol: 'database' },
];

describe('QuickConnectWidget', () => {
  const onConnect = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(mockHosts);
  });

  it('shows loading state initially', () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(<QuickConnectWidget onConnect={onConnect} />);
    expect(screen.getByText(/Loading hosts/i)).toBeInTheDocument();
  });

  it('renders saved hosts after loading', async () => {
    render(<QuickConnectWidget onConnect={onConnect} />);
    await waitFor(() => {
      expect(screen.getByText('Prod Web')).toBeInTheDocument();
      expect(screen.getByText('Dev Box')).toBeInTheDocument();
    });
  });

  it('shows empty state when no hosts are saved', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<QuickConnectWidget onConnect={onConnect} />);
    await waitFor(() => {
      expect(screen.getByText(/No saved hosts/i)).toBeInTheDocument();
    });
  });

  it('shows search empty state when no hosts match', async () => {
    render(<QuickConnectWidget onConnect={onConnect} />);
    await waitFor(() => expect(screen.getByText('Prod Web')).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Search hosts/i), {
      target: { value: 'nonexistent' },
    });

    expect(screen.getByText(/No hosts found/i)).toBeInTheDocument();
  });

  it('filters hosts by search query', async () => {
    render(<QuickConnectWidget onConnect={onConnect} />);
    await waitFor(() => expect(screen.getByText('Prod Web')).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Search hosts/i), {
      target: { value: 'Dev' },
    });

    expect(screen.getByText('Dev Box')).toBeInTheDocument();
    expect(screen.queryByText('Prod Web')).not.toBeInTheDocument();
  });

  it('calls onConnect with the correct host when a host button is clicked', async () => {
    render(<QuickConnectWidget onConnect={onConnect} />);
    await waitFor(() => expect(screen.getByText('Prod Web')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Prod Web').closest('button')!);

    expect(onConnect).toHaveBeenCalledWith(mockHosts[0]);
  });

  it('shows host username and address', async () => {
    render(<QuickConnectWidget onConnect={onConnect} />);
    await waitFor(() => {
      expect(screen.getByText(/root@10\.0\.0\.1/)).toBeInTheDocument();
    });
  });

  it('shows "+N more hosts" when more than 6 hosts exist', async () => {
    const manyHosts = Array.from({ length: 8 }, (_, i) => ({
      id: String(i + 1),
      name: `Host ${i + 1}`,
      address: `10.0.0.${i + 1}`,
      port: 22,
      username: 'root',
      protocol: 'ssh',
    }));
    mockInvoke.mockResolvedValue(manyHosts);

    render(<QuickConnectWidget onConnect={onConnect} />);
    await waitFor(() => {
      expect(screen.getByText(/\+2 more hosts/)).toBeInTheDocument();
    });
  });
});
