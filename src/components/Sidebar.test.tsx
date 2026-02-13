import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Sidebar from './Sidebar';
import '@testing-library/jest-dom';

describe('Sidebar', () => {
  const mockOnViewChange = vi.fn();

  it('renders all navigation items', () => {
    render(<Sidebar activeView="dashboard" onViewChange={mockOnViewChange} />);

    expect(screen.getByText('Dashboard')).toBeInTheDocument();
    expect(screen.getByText('Remote')).toBeInTheDocument();
    expect(screen.getByText('Monitoring')).toBeInTheDocument();
    expect(screen.getByText('AI Assistant')).toBeInTheDocument();
    expect(screen.getByText('Security')).toBeInTheDocument();
    expect(screen.getByText('Settings')).toBeInTheDocument();
  });

  it('highlights the active view', () => {
    render(<Sidebar activeView="monitoring" onViewChange={mockOnViewChange} />);

    const monitoringButton = screen.getByText('Monitoring').closest('button');
    expect(monitoringButton?.className).toContain('text-accent');
  });

  it('calls onViewChange when a nav item is clicked', () => {
    render(<Sidebar activeView="dashboard" onViewChange={mockOnViewChange} />);

    fireEvent.click(screen.getByText('Remote'));
    expect(mockOnViewChange).toHaveBeenCalledWith('remote');

    fireEvent.click(screen.getByText('Security'));
    expect(mockOnViewChange).toHaveBeenCalledWith('security');
  });

  it('renders app branding', () => {
    render(<Sidebar activeView="dashboard" onViewChange={mockOnViewChange} />);

    expect(screen.getByText('QUASAR')).toBeInTheDocument();
    expect(screen.getByText('v0.1.0-alpha')).toBeInTheDocument();
  });
});
