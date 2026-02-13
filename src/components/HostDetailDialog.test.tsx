import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import HostDetailDialog from './HostDetailDialog';
import '@testing-library/jest-dom';

const mockHost = {
  ip: '192.168.1.10',
  is_alive: true,
  latency_ms: 5,
  open_ports: [22, 80, 443],
  hostname: 'web-server',
  mac_address: 'AA:BB:CC:DD:EE:FF',
  device_type: 'server',
  services: [
    { port: 22, protocol: 'tcp', service: 'ssh', version: 'OpenSSH 8.9' },
    { port: 80, protocol: 'tcp', service: 'http' },
    { port: 443, protocol: 'tcp', service: 'https' },
  ],
  vendor: 'Dell',
  last_seen: Math.floor(Date.now() / 1000),
};

describe('HostDetailDialog', () => {
  it('returns null when host is null', () => {
    const { container } = render(
      <HostDetailDialog host={null} onClose={vi.fn()} />
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders host details when host is provided', () => {
    render(<HostDetailDialog host={mockHost} onClose={vi.fn()} />);

    expect(screen.getAllByText('192.168.1.10').length).toBeGreaterThan(0);
    expect(screen.getAllByText('web-server').length).toBeGreaterThan(0);
  });

  it('renders overview tab by default', () => {
    render(<HostDetailDialog host={mockHost} onClose={vi.fn()} />);

    expect(screen.getByText('Overview')).toBeInTheDocument();
    expect(screen.getByText(/Services/)).toBeInTheDocument();
    expect(screen.getByText('Actions')).toBeInTheDocument();
  });

  it('switches to services tab', () => {
    render(<HostDetailDialog host={mockHost} onClose={vi.fn()} />);

    fireEvent.click(screen.getByText(/Services/));
    expect(screen.getByText('ssh')).toBeInTheDocument();
    expect(screen.getByText('OpenSSH 8.9')).toBeInTheDocument();
  });

  it('switches to actions tab', () => {
    render(
      <HostDetailDialog
        host={mockHost}
        onClose={vi.fn()}
        onConnect={vi.fn()}
        onSave={vi.fn()}
        onDelete={vi.fn()}
      />
    );

    fireEvent.click(screen.getByText('Actions'));
    expect(screen.getByText(/connect/i)).toBeInTheDocument();
  });

  it('calls onClose when close button is clicked', () => {
    const onClose = vi.fn();
    const { container } = render(<HostDetailDialog host={mockHost} onClose={onClose} />);

    // The close button is the one with the X icon in the header
    const closeBtn = container.querySelector('.lucide-x')?.closest('button');
    if (closeBtn) fireEvent.click(closeBtn);

    expect(onClose).toHaveBeenCalled();
  });

  it('calls onConnect when connect action is triggered', () => {
    const onConnect = vi.fn();
    render(
      <HostDetailDialog
        host={mockHost}
        onClose={vi.fn()}
        onConnect={onConnect}
      />
    );

    fireEvent.click(screen.getByText('Actions'));
    const connectBtns = screen.getAllByText(/connect/i);
    fireEvent.click(connectBtns[0]);

    expect(onConnect).toHaveBeenCalledWith(mockHost);
  });
});
