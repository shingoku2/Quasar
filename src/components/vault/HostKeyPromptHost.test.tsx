import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { HostKeyPromptHost } from './HostKeyPromptHost';
import { useSshHostKeyVerification } from '../../hooks/useSshHostKeyVerification';
import '@testing-library/jest-dom';

vi.mock('../../hooks/useSshHostKeyVerification', () => ({
  useSshHostKeyVerification: vi.fn(),
}));

const changedPrompt = (host: string) => ({
  host,
  port: 22,
  fingerprint: `SHA256:new-${host}`,
  keyType: 'ssh-ed25519',
  isChanged: true,
  oldFingerprint: `SHA256:old-${host}`,
});

const hookState = (requestId: string, host: string) => ({
  promptData: changedPrompt(host),
  promptRequestId: requestId,
  handleTrust: vi.fn(),
  handleReject: vi.fn(),
});

describe('HostKeyPromptHost', () => {
  beforeEach(() => {
    vi.mocked(useSshHostKeyVerification).mockReset();
  });

  it('renders nothing without a pending prompt', () => {
    vi.mocked(useSshHostKeyVerification).mockReturnValue({
      promptData: null,
      promptRequestId: null,
      handleTrust: vi.fn(),
      handleReject: vi.fn(),
    });
    const { container } = render(<HostKeyPromptHost />);
    expect(container).toBeEmptyDOMElement();
  });

  // PR #68 review: acknowledging one changed key must not pre-acknowledge the next queued one.
  it('starts each queued changed-key prompt unverified', () => {
    vi.mocked(useSshHostKeyVerification).mockReturnValue(hookState('req-1', 'a.example'));
    const { rerender } = render(<HostKeyPromptHost />);
    fireEvent.click(screen.getByRole('checkbox', { name: /I verified the new fingerprint/i }));
    expect(screen.getByRole('button', { name: /Accept New Key & Connect/i })).toBeEnabled();

    vi.mocked(useSshHostKeyVerification).mockReturnValue(hookState('req-2', 'b.example'));
    rerender(<HostKeyPromptHost />);
    expect(screen.getByRole('checkbox', { name: /I verified the new fingerprint/i })).not.toBeChecked();
    expect(screen.getByRole('button', { name: /Accept New Key & Connect/i })).toBeDisabled();
  });
});
