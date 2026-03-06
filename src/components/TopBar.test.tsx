import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import TopBar from './TopBar';
import '@testing-library/jest-dom';

describe('TopBar', () => {
  it('renders the correct label for each view', () => {
    const views = [
      { id: 'dashboard' as const, label: 'Dashboard' },
      { id: 'remote' as const, label: 'Remote' },
      { id: 'monitoring' as const, label: 'Monitoring' },
      { id: 'ai' as const, label: 'AI Assistant' },
      { id: 'automation' as const, label: 'Automation' },
      { id: 'security' as const, label: 'Security' },
      { id: 'settings' as const, label: 'Settings' },
    ] as const;

    for (const view of views) {
      const { unmount } = render(<TopBar activeView={view.id} />);
      expect(screen.getByRole('heading', { name: view.label })).toBeInTheDocument();
      unmount();
    }
  });

  it('renders a search input', () => {
    render(<TopBar activeView="dashboard" />);
    expect(screen.getByPlaceholderText(/Search hosts, credentials/i)).toBeInTheDocument();
  });
});
