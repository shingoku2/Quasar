import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import NetworkMapWidget from './NetworkMapWidget';
import '@testing-library/jest-dom';

describe('NetworkMapWidget', () => {
  it('renders empty state when no hosts', () => {
    render(<NetworkMapWidget />);
    
    expect(screen.getByText('Network Map')).toBeInTheDocument();
    expect(screen.getByText('No network data available.')).toBeInTheDocument();
  });

  it('displays online and offline counts', () => {
    const hosts = [
      { id: '1', ip: '192.168.1.10', type: 'server' as const, status: 'online' as const },
      { id: '2', ip: '192.168.1.20', type: 'workstation' as const, status: 'offline' as const },
      { id: '3', ip: '192.168.1.30', type: 'server' as const, status: 'online' as const }
    ];
    
    render(<NetworkMapWidget hosts={hosts} />);
    
    expect(screen.getByText('2 Online')).toBeInTheDocument();
    expect(screen.getByText('1 Offline')).toBeInTheDocument();
  });

  it('limits display to 6 hosts with overflow message', () => {
    const hosts = Array.from({ length: 10 }, (_, i) => ({
      id: String(i),
      ip: `192.168.1.${i}`,
      type: 'workstation' as const,
      status: 'online' as const
    }));
    
    render(<NetworkMapWidget hosts={hosts} />);
    
    expect(screen.getByText('+4 more hosts')).toBeInTheDocument();
  });
});
