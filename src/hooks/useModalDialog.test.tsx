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

  // Review: a native input with tabIndex -1 (like the unlock dialog's hidden username field)
  // is skipped by Tab, so it must not count as the trap's first item.
  it('skips negative-tabindex fields when wrapping', () => {
    const WithHiddenField: React.FC = () => {
      const ref = useModalDialog();
      return (
        <div role="dialog" aria-modal="true" aria-label="Unlock" ref={ref} tabIndex={-1}>
          <input aria-label="Hidden username" tabIndex={-1} />
          <input aria-label="Password" />
          <button>Unlock</button>
        </div>
      );
    };
    render(<WithHiddenField />);
    const password = screen.getByLabelText('Password');
    expect(password).toHaveFocus();
    fireEvent.keyDown(password, { key: 'Tab', shiftKey: true });
    expect(screen.getByRole('button', { name: 'Unlock' })).toHaveFocus();
  });

  // Review: with an autoFocus field, the opener must still get focus back on close.
  it('gives focus back to the opener when the dialog autofocuses a field', () => {
    const AutoFocusDialog: React.FC<{ onClose: () => void }> = ({ onClose }) => {
      const ref = useModalDialog(onClose);
      return (
        <div role="dialog" aria-modal="true" aria-label="Prompt" ref={ref} tabIndex={-1}>
          <input aria-label="Password" autoFocus />
        </div>
      );
    };
    const Opener: React.FC = () => {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button onClick={() => setOpen(true)}>Open</button>
          {open && <AutoFocusDialog onClose={() => setOpen(false)} />}
        </>
      );
    };
    render(<Opener />);
    const openButton = screen.getByRole('button', { name: 'Open' });
    openButton.focus();
    fireEvent.click(openButton);
    const password = screen.getByLabelText('Password');
    expect(password).toHaveFocus();
    fireEvent.keyDown(password, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(openButton).toHaveFocus();
  });

  // Review: a dialog that replaces another in one render (selector -> manual prompt)
  // returns focus to the original opener, not to the vanished button inside the first.
  it('carries the opener across a dialog handoff', () => {
    const Step: React.FC<{ label: string; onNext?: () => void; onClose: () => void }> = ({ label, onNext, onClose }) => {
      const ref = useModalDialog(onClose);
      return (
        <div role="dialog" aria-modal="true" aria-label={label} ref={ref} tabIndex={-1}>
          {onNext ? <button onClick={onNext}>Enter manually</button> : <input aria-label="Password" />}
        </div>
      );
    };
    const Flow: React.FC = () => {
      const [step, setStep] = useState<'none' | 'select' | 'manual'>('none');
      return (
        <>
          <button onClick={() => setStep('select')}>Connect</button>
          {step === 'select' && <Step label="Select" onNext={() => setStep('manual')} onClose={() => setStep('none')} />}
          {step === 'manual' && <Step label="Manual" onClose={() => setStep('none')} />}
        </>
      );
    };
    render(<Flow />);
    const connect = screen.getByRole('button', { name: 'Connect' });
    connect.focus();
    fireEvent.click(connect);
    const manual = screen.getByRole('button', { name: 'Enter manually' });
    expect(manual).toHaveFocus();
    fireEvent.click(manual);
    const password = screen.getByLabelText('Password');
    expect(password).toHaveFocus();
    fireEvent.keyDown(password, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(connect).toHaveFocus();
  });

  // Self-review: the prompt that takes over from the selector autofocuses its field; the
  // selector's close must not pull focus away to the prompt's first control (its X button).
  it('keeps an autofocused field focused through a dialog handoff', () => {
    const Selector: React.FC<{ onManual: () => void; onClose: () => void }> = ({ onManual, onClose }) => {
      const ref = useModalDialog(onClose);
      return (
        <div role="dialog" aria-modal="true" aria-label="Select" ref={ref} tabIndex={-1}>
          <button onClick={onManual}>Enter manually</button>
        </div>
      );
    };
    const Prompt: React.FC<{ onClose: () => void }> = ({ onClose }) => {
      const ref = useModalDialog(onClose);
      return (
        <div role="dialog" aria-modal="true" aria-label="Prompt" ref={ref} tabIndex={-1}>
          <button aria-label="Close dialog" onClick={onClose}>x</button>
          <input aria-label="Password" autoFocus />
        </div>
      );
    };
    const Flow: React.FC = () => {
      const [step, setStep] = useState<'none' | 'select' | 'manual'>('none');
      return (
        <>
          <button onClick={() => setStep('select')}>Connect</button>
          {step === 'select' && <Selector onManual={() => setStep('manual')} onClose={() => setStep('none')} />}
          {step === 'manual' && <Prompt onClose={() => setStep('none')} />}
        </>
      );
    };
    render(<Flow />);
    const connect = screen.getByRole('button', { name: 'Connect' });
    connect.focus();
    fireEvent.click(connect);
    fireEvent.click(screen.getByRole('button', { name: 'Enter manually' }));
    expect(screen.getByLabelText('Password')).toHaveFocus();
    fireEvent.keyDown(screen.getByLabelText('Password'), { key: 'Escape' });
    expect(connect).toHaveFocus();
  });

  // Self-review: StrictMode (main.tsx) runs effect cleanup + setup twice on mount; the
  // simulated cleanup must not move focus off the autofocused field.
  it('keeps an autofocused field focused under StrictMode', () => {
    const Prompt: React.FC = () => {
      const ref = useModalDialog(() => {});
      return (
        <div role="dialog" aria-modal="true" aria-label="Prompt" ref={ref} tabIndex={-1}>
          <button aria-label="Close dialog">x</button>
          <input aria-label="Password" autoFocus />
        </div>
      );
    };
    const Opener: React.FC = () => {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button onClick={() => setOpen(true)}>Open</button>
          {open && <Prompt />}
        </>
      );
    };
    render(<React.StrictMode><Opener /></React.StrictMode>);
    const openButton = screen.getByRole('button', { name: 'Open' });
    openButton.focus();
    fireEvent.click(openButton);
    expect(screen.getByLabelText('Password')).toHaveFocus();
  });

  it('skips disabled controls that carry a tabindex', () => {
    const WithDisabled: React.FC = () => {
      const ref = useModalDialog();
      return (
        <div role="dialog" aria-modal="true" aria-label="D" ref={ref} tabIndex={-1}>
          <input aria-label="Name" />
          <button disabled tabIndex={0}>Save</button>
        </div>
      );
    };
    render(<WithDisabled />);
    const name = screen.getByLabelText('Name');
    expect(name).toHaveFocus();
    // Name is the only tabbable control, so Tab must wrap (be trapped), not leave the dialog.
    expect(fireEvent.keyDown(name, { key: 'Tab' })).toBe(false);
    expect(name).toHaveFocus();
  });
});
