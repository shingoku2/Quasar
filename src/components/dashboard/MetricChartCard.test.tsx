import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import MetricChartCard from './MetricChartCard';
import '@testing-library/jest-dom';

// Mock Recharts because it uses DOM measurements that JSDOM doesn't support
vi.mock('recharts', () => ({
  ResponsiveContainer: ({ children }: any) => <div data-testid="responsive-container">{children}</div>,
  AreaChart: ({ children, data }: any) => <svg data-testid="area-chart" data-chartdata={JSON.stringify(data)}>{children}</svg>,
  Area: () => <path data-testid="area" />,
  CartesianGrid: () => <g data-testid="cartesian-grid" />,
  Tooltip: () => <div data-testid="tooltip" />,
}));

describe('MetricChartCard', () => {
  const defaultProps = {
    title: 'CPU Usage',
    value: '45%',
    data: [
      { time: '10:00', value: 20 },
      { time: '10:05', value: 45 },
    ],
  };

  it('renders the title and value correctly', () => {
    render(<MetricChartCard {...defaultProps} />);
    expect(screen.getByText('CPU Usage')).toBeInTheDocument();
    expect(screen.getByText('45%')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(<MetricChartCard {...defaultProps} className="custom-test-class" />);
    // The main wrapper is the first div
    expect(container.firstChild).toHaveClass('custom-test-class');
  });

  it('renders with default color and uses it in pulse indicator', () => {
    const { container } = render(<MetricChartCard {...defaultProps} />);
    const pulseDiv = container.querySelector('.animate-pulse');
    expect(pulseDiv).toBeInTheDocument();
    // Default color is #3b82f6 (blue)
    expect(pulseDiv).toHaveStyle({ backgroundColor: '#3b82f6' });
  });

  it('uses custom color prop in pulse indicator', () => {
    const { container } = render(<MetricChartCard {...defaultProps} color="#ff0000" />);
    const pulseDiv = container.querySelector('.animate-pulse');
    expect(pulseDiv).toBeInTheDocument();
    expect(pulseDiv).toHaveStyle({ backgroundColor: '#ff0000' });
  });

  it('passes data to the chart component', () => {
    render(<MetricChartCard {...defaultProps} />);
    const chart = screen.getByTestId('area-chart');
    expect(chart).toBeInTheDocument();
    expect(chart).toHaveAttribute('data-chartdata', JSON.stringify(defaultProps.data));
  });

  it('generates a valid gradient id based on the title', () => {
    // Tests that special characters in the title are replaced correctly for the gradient id
    const propsWithSpecialChars = {
      ...defaultProps,
      title: 'CPU / RAM (Usage)!',
    };

    render(<MetricChartCard {...propsWithSpecialChars} />);

    // Check that there is a linearGradient with an id that doesn't contain spaces or slashes
    // The actual component uses: `color-${title.replace(/[^a-zA-Z0-9_-]/g, '-')}`
    // 'CPU / RAM (Usage)!' -> 'color-CPU---RAM--Usage--'

    // We mock recharts, but defs/linearGradient/stop are standard SVG elements and get rendered
    const gradient = document.querySelector('linearGradient');
    expect(gradient).toBeInTheDocument();
    expect(gradient?.id).toBe('color-CPU---RAM--Usage--');
  });
});
