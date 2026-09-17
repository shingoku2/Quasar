import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import CredentialPrompt from './CredentialPrompt';
import '@testing-library/jest-dom';

describe('CredentialPrompt', () => {
  it('renders correctly', () => {
    const onCancel = vi.fn();
    const onSubmit = vi.fn();

    render(
      <CredentialPrompt
        hostName="example.com"
        initialUsername="testuser"
        onCancel={onCancel}
        onSubmit={onSubmit}
      />
    );

    expect(screen.getByText(/Authentication Required/i)).toBeInTheDocument();
    expect(screen.getByText('testuser')).toBeInTheDocument();
    expect(screen.getByText('example.com')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Username')).toHaveValue('testuser');
    expect(screen.getByPlaceholderText('Password')).toBeInTheDocument();
  });

  it('calls onSubmit with username and password when form is submitted', async () => {
    const onCancel = vi.fn();
    const onSubmit = vi.fn();

    render(
      <CredentialPrompt
        hostName="example.com"
        onCancel={onCancel}
        onSubmit={onSubmit}
      />
    );

    const usernameInput = screen.getByPlaceholderText('Username');
    const passwordInput = screen.getByPlaceholderText('Password');
    const submitButton = screen.getByText('Connect Session');

    fireEvent.change(usernameInput, { target: { value: 'newuser' } });
    fireEvent.change(passwordInput, { target: { value: 'secretpassword' } });
    fireEvent.click(submitButton);

    expect(onSubmit).toHaveBeenCalledWith('newuser', 'secretpassword', {
      saveCredential: false,
      credentialName: undefined,
    });
  });

  it('calls onCancel when cancel button is clicked', () => {
    const onCancel = vi.fn();
    const onSubmit = vi.fn();

    render(
      <CredentialPrompt
        hostName="example.com"
        onCancel={onCancel}
        onSubmit={onSubmit}
      />
    );

    const cancelButton = screen.getByText('Cancel');
    fireEvent.click(cancelButton);

    expect(onCancel).toHaveBeenCalled();
  });

  it('handles save credential functionality when allowSaveCredential is true', () => {
    const onCancel = vi.fn();
    const onSubmit = vi.fn();

    render(
      <CredentialPrompt
        hostName="example.com"
        allowSaveCredential={true}
        onCancel={onCancel}
        onSubmit={onSubmit}
      />
    );

    const saveCheckbox = screen.getByLabelText(/Save these credentials to Security vault/i);
    expect(saveCheckbox).toBeInTheDocument();

    // Check the box
    fireEvent.click(saveCheckbox);

    const credentialNameInput = screen.getByPlaceholderText(/e.g. example.com \(user\)/i);
    expect(credentialNameInput).toBeInTheDocument();

    const usernameInput = screen.getByPlaceholderText('Username');
    const passwordInput = screen.getByPlaceholderText('Password');
    const submitButton = screen.getByText('Connect Session');

    fireEvent.change(usernameInput, { target: { value: 'vaultuser' } });
    fireEvent.change(passwordInput, { target: { value: 'vaultpassword' } });
    fireEvent.change(credentialNameInput, { target: { value: 'My Credential' } });
    fireEvent.click(submitButton);

    expect(onSubmit).toHaveBeenCalledWith('vaultuser', 'vaultpassword', {
      saveCredential: true,
      credentialName: 'My Credential',
    });
  });

  it('does not submit if username or password is empty', () => {
    const onCancel = vi.fn();
    const onSubmit = vi.fn();

    render(
      <CredentialPrompt
        hostName="example.com"
        onCancel={onCancel}
        onSubmit={onSubmit}
      />
    );

    const submitButton = screen.getByText('Connect Session');

    // Neither filled
    fireEvent.click(submitButton);
    expect(onSubmit).not.toHaveBeenCalled();

    // Only username filled
    const usernameInput = screen.getByPlaceholderText('Username');
    fireEvent.change(usernameInput, { target: { value: 'user' } });
    fireEvent.click(submitButton);
    expect(onSubmit).not.toHaveBeenCalled();

    // Only password filled
    fireEvent.change(usernameInput, { target: { value: '' } });
    const passwordInput = screen.getByPlaceholderText('Password');
    fireEvent.change(passwordInput, { target: { value: 'pass' } });
    fireEvent.click(submitButton);
    expect(onSubmit).not.toHaveBeenCalled();

    // Whitespace-only username satisfies the native `required` attribute but
    // must still fail the component's own username.trim() guard.
    fireEvent.change(usernameInput, { target: { value: '   ' } });
    fireEvent.click(submitButton);
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
