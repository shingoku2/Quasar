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
  it('renders system health widget', () => {
    render(<DashboardView />);
    expect(screen.getByText('SYSTEMS NOMINAL')).toBeInTheDocument();
    expect(screen.getByText('98%')).toBeInTheDocument();
  });

  it('renders metric cards', () => {
    render(<DashboardView />);
    expect(screen.getByText('CPU Usage')).toBeInTheDocument();
    expect(screen.getByText('Memory Utilization')).toBeInTheDocument();
  });

  it('renders alert feed', () => {
    render(<DashboardView />);
    expect(screen.getByText('Recent Alerts')).toBeInTheDocument();
    expect(screen.getAllByText('Database-01').length).toBeGreaterThan(0);
  });
});
