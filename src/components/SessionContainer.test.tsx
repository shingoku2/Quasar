import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import SessionContainer, { SessionTab } from './SessionContainer';
import '@testing-library/jest-dom';

describe('SessionContainer', () => {
  const mockTabs: SessionTab[] = [
    { id: '1', title: 'Tab 1', content: <div>Content 1</div> },
    { id: '2', title: 'Tab 2', content: <div>Content 2</div> },
    { id: '3', title: 'Tab 3', content: <div>Content 3</div> },
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

  it('supports split view', () => {
    render(
      <SessionContainer 
        tabs={mockTabs} 
        activeTabId="1"
        splitViewIds={['1', '2']} 
        onTabChange={() => {}} 
        onTabClose={() => {}} 
      />
    );
    // Both 1 and 2 should be visible
    expect(screen.getByText('Content 1')).toBeInTheDocument();
    expect(screen.getByText('Content 2')).toBeInTheDocument();
    
    // 3 should be missing (not rendered in split mode)
    expect(screen.queryByText('Content 3')).not.toBeInTheDocument();
  });
});