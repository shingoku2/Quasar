import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import SessionContainer, { SessionTab } from './SessionContainer';
import '@testing-library/jest-dom';

describe('SessionContainer', () => {
  const mockTabs: SessionTab[] = [
    { id: '1', title: 'Tab 1', content: <div>Content 1</div> },
    { id: '2', title: 'Tab 2', content: <div>Content 2</div> },
  ];

  it('renders tabs correctly', () => {
    render(
      <SessionContainer 
        tabs={mockTabs} 
        activeTabId="1" 
        onTabChange={() => {}} 
        onTabClose={() => {}} 
      />
    );
    expect(screen.getByText('Tab 1')).toBeInTheDocument();
    expect(screen.getByText('Tab 2')).toBeInTheDocument();
  });

  it('shows active tab content', () => {
    render(
      <SessionContainer 
        tabs={mockTabs} 
        activeTabId="1" 
        onTabChange={() => {}} 
        onTabClose={() => {}} 
      />
    );
    expect(screen.getByText('Content 1').parentElement).not.toHaveClass('hidden');
    expect(screen.getByText('Content 2').parentElement).toHaveClass('hidden');
  });

  it('calls onTabChange when a tab is clicked', () => {
    const handleTabChange = vi.fn();
    render(
      <SessionContainer 
        tabs={mockTabs} 
        activeTabId="1" 
        onTabChange={handleTabChange} 
        onTabClose={() => {}} 
      />
    );
    
    fireEvent.click(screen.getByText('Tab 2'));
    expect(handleTabChange).toHaveBeenCalledWith('2');
  });

  it('calls onTabClose when close button is clicked', () => {
    const handleTabClose = vi.fn();
    render(
      <SessionContainer 
        tabs={mockTabs} 
        activeTabId="1" 
        onTabChange={() => {}} 
        onTabClose={handleTabClose} 
      />
    );
    
    // Find the close button for Tab 2. 
    // We assume the close icon is within the tab container.
    // A simple way is to look for a button within the tab element or by test id.
    const closeButtons = screen.getAllByRole('button');
    // Assuming 2 close buttons (one for each tab if they are closable)
    // We'll adjust the component to ensure accessibility
    fireEvent.click(closeButtons[1]); 
    expect(handleTabClose).toHaveBeenCalledWith('2');
  });
});
