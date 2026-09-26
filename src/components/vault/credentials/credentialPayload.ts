import type { Credential, CredentialFormData } from './types';

export type CredentialPayload =
  | { ok: true; command: 'add_credential' | 'update_credential'; args: Record<string, unknown> }
  | { ok: false; error: string };

/** The form as it opens: blank for a new credential, the stored fields (never secrets) for an edit. */
export function initialCredentialForm(credential?: Credential): CredentialFormData {
  return {
    name: credential?.name || '',
    username: credential?.username || '',
    // Never prefilled: get_credential doesn't return it. Blank on edit means "keep".
    password: '',
    credential_type: credential?.credential_type || 'ssh',
    host: credential?.host || '',
    port: credential?.port?.toString() ?? '',
    key_path: credential?.key_path || '',
    private_key: '',
    key_passphrase: '',
  };
}

/**
 * Validates the form and builds the `add_credential` / `update_credential` call.
 *
 * Keys must be camelCase: Tauri matches invoke args against the camelCased Rust
 * parameter names and silently ignores unknown keys, so a snake_case key would make
 * the field never update. On edit, `null` means "keep the stored value" and `''`
 * means "clear it".
 */
export function buildCredentialPayload(form: CredentialFormData, existing?: Credential): CredentialPayload {
  const isKey = form.credential_type === 'ssh_key';

  // get_credential never returns decrypted key material, so an existing ssh_key
  // credential's private_key field is always blank in the form — that must not
  // be mistaken for "no key material provided" or every edit gets blocked.
  const hasStoredKeyMaterial = Boolean(existing?.key_path) || Boolean(existing?.has_private_key);
  if (isKey && !form.key_path.trim() && !form.private_key.trim() && !hasStoredKeyMaterial) {
    return { ok: false, error: 'Provide either key path or paste private key PEM.' };
  }

  const port = form.port === '' ? null : Number(form.port);
  if (port !== null && (!Number.isInteger(port) || port < 1 || port > 65535)) {
    return { ok: false, error: 'Port must be between 1 and 65535.' };
  }

  if (!existing) {
    return {
      ok: true,
      command: 'add_credential',
      args: {
        name: form.name,
        username: form.username,
        password: isKey ? '' : form.password,
        credentialType: form.credential_type,
        host: form.host || null,
        port,
        metadata: null,
        keyPath: isKey && form.key_path ? form.key_path : null,
        privateKey: isKey && form.private_key ? form.private_key : null,
        keyPassphrase: isKey && form.key_passphrase ? form.key_passphrase : null,
      },
    };
  }

  const args: Record<string, unknown> = {
    credentialId: existing.id,
    name: form.name,
    username: form.username,
    metadata: JSON.stringify({
      credential_type: form.credential_type,
      host: form.host || null,
      port,
    }),
    credentialType: form.credential_type,
    host: form.host,
    port,
  };
  if (isKey) {
    args.keyPath = form.key_path;
    // A blank key field keeps the stored key (the backend only updates on non-null).
    args.privateKey = form.private_key || null;
    args.keyPassphrase = form.key_passphrase || null;
    // Switching to a key clears the stored password.
    args.password = '';
  } else {
    args.password = form.password || null;
    // Switching to a password clears the stored key fields.
    args.keyPath = '';
    args.privateKey = '';
    args.keyPassphrase = '';
  }
  return { ok: true, command: 'update_credential', args };
}
