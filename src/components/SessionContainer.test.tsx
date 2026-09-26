import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
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
    expect(screen.getByText('Content 1').parentElement).not.toHaveClass('hidden');
    expect(screen.getByText('Content 2').parentElement).not.toHaveClass('hidden');

    // 3 stays mounted (not part of the split set) so its underlying session
    // isn't torn down, but it must be hidden from view.
    expect(screen.getByText('Content 3').parentElement).toHaveClass('hidden');
  });
});
describe('SessionContainer interactions', () => {
  const tabs: SessionTab[] = [
    { id: 'inv', title: 'Inventory', content: <div>Inventory body</div>, closable: false },
    { id: 'a', title: 'Alpha', content: <div>Alpha body</div> },
    { id: 'b', title: 'Beta', content: <div>Beta body</div> },
  ];

  it('clicking a tab selects it', () => {
    const onTabChange = vi.fn();
    render(<SessionContainer tabs={tabs} activeTabId="inv" onTabChange={onTabChange} onTabClose={vi.fn()} />);
    fireEvent.click(screen.getByText('Beta'));
    expect(onTabChange).toHaveBeenCalledWith('b');
  });

  it('closing a tab does not also select it', () => {
    const onTabChange = vi.fn();
    const onTabClose = vi.fn();
    render(<SessionContainer tabs={tabs} activeTabId="inv" onTabChange={onTabChange} onTabClose={onTabClose} />);
    fireEvent.click(screen.getByLabelText('Close Alpha'));
    expect(onTabClose).toHaveBeenCalledWith('a');
    expect(onTabChange).not.toHaveBeenCalled();
  });

  it('a non-closable tab has neither close nor split buttons', () => {
    render(
      <SessionContainer tabs={tabs} activeTabId="inv" onTabChange={vi.fn()} onTabClose={vi.fn()} onToggleSplit={vi.fn()} />
    );
    expect(screen.queryByLabelText('Close Inventory')).not.toBeInTheDocument();
    // Only the two closable tabs get a split toggle.
    expect(screen.getAllByLabelText('Toggle Split View')).toHaveLength(2);
  });

  it('split buttons appear only when onToggleSplit is provided', () => {
    render(<SessionContainer tabs={tabs} activeTabId="a" onTabChange={vi.fn()} onTabClose={vi.fn()} />);
    expect(screen.queryByLabelText('Toggle Split View')).not.toBeInTheDocument();
  });

  it('toggling split does not also select the tab', () => {
    const onTabChange = vi.fn();
    const onToggleSplit = vi.fn();
    render(
      <SessionContainer tabs={tabs} activeTabId="a" onTabChange={onTabChange} onTabClose={vi.fn()} onToggleSplit={onToggleSplit} />
    );
    fireEvent.click(screen.getAllByLabelText('Toggle Split View')[1]);
    expect(onToggleSplit).toHaveBeenCalledWith('b');
    expect(onTabChange).not.toHaveBeenCalled();
  });

  it('split mode lays out one grid column per visible tab', () => {
    const { container } = render(
      <SessionContainer tabs={tabs} activeTabId="a" splitViewIds={['a', 'b']} onTabChange={vi.fn()} onTabClose={vi.fn()} onToggleSplit={vi.fn()} />
    );
    const content = screen.getByText('Alpha body').parentElement?.parentElement as HTMLElement;
    expect(content.style.gridTemplateColumns).toBe('repeat(2, 1fr)');
    expect(screen.getByText('Inventory body').parentElement).toHaveClass('hidden');
    // Tabs in the split set get the highlighted toggle.
    const toggles = screen.getAllByLabelText('Toggle Split View');
    toggles.forEach((t) => expect(t).toHaveClass('text-accent'));
    expect(container.querySelectorAll('.h-0\\.5.bg-accent')).toHaveLength(1);
  });

  it('switching the active tab keeps every tab mounted', () => {
    const { rerender } = render(<SessionContainer tabs={tabs} activeTabId="a" onTabChange={vi.fn()} onTabClose={vi.fn()} />);
    rerender(<SessionContainer tabs={tabs} activeTabId="b" onTabChange={vi.fn()} onTabClose={vi.fn()} />);
    expect(screen.getByText('Alpha body').parentElement).toHaveClass('hidden');
    expect(screen.getByText('Beta body').parentElement).toHaveClass('block');
  });
});
