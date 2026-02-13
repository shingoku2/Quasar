import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import DashboardView from './DashboardView';
import '@testing-library/jest-dom';

vi.mock('../NetworkTopologyView', () => ({
  default: () => <div data-testid="topology-mock">Topology Mock</div>,
}));

// Mock Recharts because it uses DOM measurements that JSDOM doesn't support
vi.mock('recharts', () => ({
  ResponsiveContainer: ({ children }: any) => <div>{children}</div>,
  AreaChart: ({ children }: any) => <div>{children}</div>,
  Area: () => <div />,
  XAxis: () => <div />,
  YAxis: () => <div />,
  CartesianGrid: () => <div />,
  Tooltip: () => <div />,
  LineChart: ({ children }: any) => <div>{children}</div>,
  Line: () => <div />,
}));

describe('DashboardView', () => {
  const mockNavigate = vi.fn();

  it('renders real-time metrics widget', () => {
    render(<DashboardView onNavigate={mockNavigate} />);
    expect(screen.getByText(/real-time metrics/i)).toBeInTheDocument();
  });

  it('renders network topology section', () => {
    render(<DashboardView onNavigate={mockNavigate} />);
    expect(screen.getByText('Network Topology')).toBeInTheDocument();
  });

  it('renders view mode toggle', () => {
    render(<DashboardView onNavigate={mockNavigate} />);
    expect(screen.getByText('List')).toBeInTheDocument();
    expect(screen.getByText('Topology')).toBeInTheDocument();
  });
});
