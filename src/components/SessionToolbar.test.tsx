import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import SessionToolbar from './SessionToolbar';
import '@testing-library/jest-dom';

describe('SessionToolbar', () => {
  it('renders correctly', () => {
    render(
      <SessionToolbar 
        sessionId="test" 
        latency={50} 
        bandwidth="1.2 Mbps" 
        clipboardSync={false} 
        onToggleClipboard={() => {}} 
      />
    );
    expect(screen.getByText('50ms')).toBeInTheDocument();
    expect(screen.getByText('1.2 Mbps')).toBeInTheDocument();
  });

  it('calls onToggleClipboard when toggle is clicked', () => {
    const handleToggle = vi.fn();
    render(
      <SessionToolbar 
        sessionId="test" 
        latency={50} 
        bandwidth="1.2 Mbps" 
        clipboardSync={false} 
        onToggleClipboard={handleToggle} 
      />
    );
    
    fireEvent.click(screen.getByRole('button', { name: /clipboard/i }));
    expect(handleToggle).toHaveBeenCalled();
  });
});
