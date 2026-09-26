import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React, { useState } from 'react';
import '@testing-library/jest-dom';
import { useModalDialog } from './useModalDialog';

const Dialog: React.FC<{ onClose?: () => void }> = ({ onClose }) => {
  const ref = useModalDialog(onClose);
  return (
    <div role="dialog" aria-modal="true" aria-label="Test dialog" ref={ref} tabIndex={-1}>
      <button>First</button>
      <input aria-label="Middle" />
      <button>Last</button>
    </div>
  );
};

const Host: React.FC = () => {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)}>Open</button>
      {open && <Dialog onClose={() => setOpen(false)} />}
    </>
  );
};

describe('useModalDialog', () => {
  it('moves focus into the dialog and closes on Escape', () => {
    const onClose = vi.fn();
    render(<Dialog onClose={onClose} />);
    expect(screen.getByRole('button', { name: 'First' })).toHaveFocus();
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('ignores Escape when the prompt needs an explicit answer', () => {
    render(<Dialog />);
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('keeps Tab and Shift+Tab inside the dialog', () => {
    render(<Dialog onClose={vi.fn()} />);
    const first = screen.getByRole('button', { name: 'First' });
    const last = screen.getByRole('button', { name: 'Last' });
    last.focus();
    fireEvent.keyDown(last, { key: 'Tab' });
    expect(first).toHaveFocus();
    fireEvent.keyDown(first, { key: 'Tab', shiftKey: true });
    expect(last).toHaveFocus();
  });

  it('gives focus back to the opener when it closes', () => {
    render(<Host />);
    const opener = screen.getByRole('button', { name: 'Open' });
    opener.focus();
    fireEvent.click(opener);
    expect(screen.getByRole('button', { name: 'First' })).toHaveFocus();
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });
});
