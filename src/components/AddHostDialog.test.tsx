import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import AddHostDialog from './AddHostDialog';

describe('AddHostDialog', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  // PR #68 review: moving a host that scheduled transfers use asks through a native dialog.
  // Declining it is a no-op, not an error.
  it('keeps the dialog open without an error when the move is declined', async () => {
    vi.mocked(invoke).mockRejectedValue('Cancelled');
    const alertSpy = vi.spyOn(window, 'alert').mockImplementation(() => {});
    const onAdded = vi.fn();
    const onClose = vi.fn();
    render(
      <AddHostDialog
        onAdded={onAdded}
        onClose={onClose}
        initialValues={{ id: 'h1', name: 'box', address: 'db1.example', protocol: 'ssh', port: 22 }}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /Save Host/i }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('update_saved_host', expect.objectContaining({ hostId: 'h1' })));
    expect(alertSpy).not.toHaveBeenCalled();
    expect(onAdded).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  // FE-020: named by its heading, and Escape closes it.
  it('is labelled by its heading and closes on Escape', () => {
    const onClose = vi.fn();
    render(<AddHostDialog onAdded={vi.fn()} onClose={onClose} />);
    const dialog = screen.getByRole('dialog', { name: /Add/i });
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
