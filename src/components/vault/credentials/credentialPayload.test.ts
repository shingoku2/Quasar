import { describe, it, expect } from 'vitest';
import { buildCredentialPayload, initialCredentialForm } from './credentialPayload';
import type { Credential, CredentialFormData } from './types';

const form = (over: Partial<CredentialFormData> = {}): CredentialFormData => ({
  ...initialCredentialForm(),
  name: 'Box',
  username: 'root',
  ...over,
});

const stored: Credential = {
  id: 'c1',
  name: 'Box',
  username: 'root',
  credential_type: 'ssh',
  created_at: '2026-01-01T00:00:00Z',
  has_password: true,
  has_private_key: false,
  has_key_passphrase: false,
};

describe('buildCredentialPayload', () => {
  it('uses only camelCase keys, for add and for update', () => {
    for (const p of [buildCredentialPayload(form({ password: 'x' })), buildCredentialPayload(form(), stored)]) {
      if (!p.ok) throw new Error(p.error);
      expect(Object.keys(p.args).filter((k) => k.includes('_'))).toEqual([]);
    }
  });

  it('rejects an out-of-range or fractional port', () => {
    for (const port of ['0', '65536', '22.5']) {
      expect(buildCredentialPayload(form({ port }))).toEqual({ ok: false, error: 'Port must be between 1 and 65535.' });
    }
    const ok = buildCredentialPayload(form({ port: '2222' }));
    expect(ok.ok && ok.args.port).toBe(2222);
  });

  it('requires key material for a new key credential, but not for an edit of one that has it', () => {
    const keyForm = form({ credential_type: 'ssh_key' });
    expect(buildCredentialPayload(keyForm).ok).toBe(false);
    const withKey = { ...stored, credential_type: 'ssh_key', has_private_key: true };
    expect(buildCredentialPayload(keyForm, withKey).ok).toBe(true);
  });

  it('keeps stored secrets when the fields are left blank on edit', () => {
    const pwd = buildCredentialPayload(form(), stored);
    expect(pwd).toMatchObject({ ok: true, command: 'update_credential', args: { credentialId: 'c1', password: null } });
    const key = buildCredentialPayload(form({ credential_type: 'ssh_key' }), { ...stored, has_private_key: true });
    expect(key).toMatchObject({ ok: true, args: { privateKey: null, keyPassphrase: null } });
  });

  it('clears the other kind of secret when the type switches', () => {
    const toKey = buildCredentialPayload(form({ credential_type: 'ssh_key', private_key: 'PEM' }), stored);
    expect(toKey).toMatchObject({ ok: true, args: { password: '', privateKey: 'PEM' } });
    const toPwd = buildCredentialPayload(form({ password: 'new' }), { ...stored, credential_type: 'ssh_key' });
    expect(toPwd).toMatchObject({ ok: true, args: { password: 'new', keyPath: '', privateKey: '', keyPassphrase: '' } });
  });

  it('sends no password for a new key credential and no key fields for a new password one', () => {
    const key = buildCredentialPayload(form({ credential_type: 'ssh_key', key_path: '~/.ssh/id', password: 'ignored' }));
    expect(key).toMatchObject({ ok: true, command: 'add_credential', args: { password: '', keyPath: '~/.ssh/id', privateKey: null } });
    const pwd = buildCredentialPayload(form({ password: 'p', key_path: 'ignored' }));
    expect(pwd).toMatchObject({ ok: true, args: { password: 'p', keyPath: null, host: null } });
  });
});
