import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import DashboardView from './DashboardView';
import '@testing-library/jest-dom';

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

  it('renders system health widget', () => {
    render(<DashboardView onNavigate={mockNavigate} />);
    // System health widget renders with status text
    expect(screen.getByText(/system health/i)).toBeInTheDocument();
  });

  it('renders network scanner section', () => {
    render(<DashboardView onNavigate={mockNavigate} />);
    expect(screen.getByText('Network Scanner')).toBeInTheDocument();
  });

  it('renders view mode toggle', () => {
    render(<DashboardView onNavigate={mockNavigate} />);
    expect(screen.getByText('List View')).toBeInTheDocument();
    expect(screen.getByText('Topology View')).toBeInTheDocument();
  });
});
