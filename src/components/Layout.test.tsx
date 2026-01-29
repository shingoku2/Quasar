import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import Layout from './Layout';
import '@testing-library/jest-dom';

describe('Layout Component', () => {
  it('renders sidebar and dashboard tab', () => {
    render(<Layout />);
    expect(screen.getByText('TITAN')).toBeInTheDocument();
    expect(screen.getAllByText('Dashboard').length).toBeGreaterThan(0);
    expect(screen.getByText('Welcome to Project Titan')).toBeInTheDocument();
  });

  it('can add and switch between tabs', () => {
    render(<Layout />);
    
    // Find add tab button by title (SVG title) or class
    const addButton = screen.getByTitle('New Session');
    fireEvent.click(addButton);

    expect(screen.getByText('New Session 1')).toBeInTheDocument();
    expect(screen.getByText('Connecting to host...')).toBeInTheDocument();

    // Switch back to dashboard
    const dashboardTab = screen.getAllByText('Dashboard')[0]; // One in sidebar, one in tab bar
    fireEvent.click(dashboardTab);
    expect(screen.getByText('Welcome to Project Titan')).toBeInTheDocument();
  });

  it('can remove tabs', () => {
    render(<Layout />);
    const addButton = screen.getByTitle('New Session');
    fireEvent.click(addButton);

    expect(screen.getByText('New Session 1')).toBeInTheDocument();

    // Find remove button (it's an SVG inside a button)
    // We can use a test ID or find by role if we had one, 
    // but let's try to find the button near the text
    const removeButton = screen.getByRole('button', { name: '' }); // The SVG close button has no name
    // This is brittle, let's fix the component to be more testable if needed, 
    // but for now we'll find the button by its container
    
    const tabItem = screen.getByText('New Session 1').parentElement;
    const closeBtn = tabItem?.querySelector('button');
    if (closeBtn) {
      fireEvent.click(closeBtn);
    }

    expect(screen.queryByText('New Session 1')).not.toBeInTheDocument();
  });
});
