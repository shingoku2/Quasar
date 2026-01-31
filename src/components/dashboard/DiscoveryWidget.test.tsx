import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import DiscoveryWidget from './DiscoveryWidget';
import '@testing-library/jest-dom';

describe('DiscoveryWidget', () => {
  it('renders empty state when no hosts', () => {
    render(<DiscoveryWidget />);
    
    expect(screen.getByText('Recent Discoveries')).toBeInTheDocument();
    expect(screen.getByText('No hosts discovered yet.')).toBeInTheDocument();
  });

  it('displays discovered hosts', () => {
    const hosts = [
      { name: 'Server 1', address: '192.168.1.10', port: 22, protocol: 'ssh', discoveredAt: new Date() },
      { name: 'Workstation', address: '192.168.1.20', port: 3389, protocol: 'rdp', discoveredAt: new Date() }
    ];
    
    render(<DiscoveryWidget hosts={hosts} />);
    
    expect(screen.getByText('Server 1')).toBeInTheDocument();
    expect(screen.getByText('Workstation')).toBeInTheDocument();
    expect(screen.getByText('2 total')).toBeInTheDocument();
  });

  it('calls onAddHost when add button clicked', () => {
    const hosts = [
      { name: 'Server 1', address: '192.168.1.10', port: 22, protocol: 'ssh', discoveredAt: new Date() }
    ];
    const onAddHost = vi.fn();
    
    render(<DiscoveryWidget hosts={hosts} onAddHost={onAddHost} />);
    
    const addButton = screen.getByTitle('Add to inventory');
    addButton.click();
    
    expect(onAddHost).toHaveBeenCalledWith(hosts[0]);
  });

  it('limits to 5 recent hosts', () => {
    const hosts = Array.from({ length: 10 }, (_, i) => ({
      name: `Host ${i}`,
      address: `192.168.1.${i}`,
      port: 22,
      protocol: 'ssh' as const,
      discoveredAt: new Date()
    }));
    
    render(<DiscoveryWidget hosts={hosts} />);
    
    expect(screen.getByText('10 total')).toBeInTheDocument();
    // Should only show first 5
    expect(screen.getByText('Host 0')).toBeInTheDocument();
    expect(screen.getByText('Host 4')).toBeInTheDocument();
    expect(screen.queryByText('Host 5')).not.toBeInTheDocument();
  });
});
