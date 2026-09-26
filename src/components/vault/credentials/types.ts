export interface CredentialSummary {
  id: string;
  name: string;
  username: string;
  credential_type: string;
  host?: string;
  port?: number;
  created_at: string;
  last_used_at?: string;
}

export interface Credential extends CredentialSummary {
  key_path?: string;
  // get_credential never returns decrypted secrets (security by design) — these
  // flags are how the UI knows a secret already exists server-side. The password
  // itself is fetched only on explicit reveal/copy via reveal_credential_password.
  has_password: boolean;
  has_private_key: boolean;
  has_key_passphrase: boolean;
}

export interface CredentialFormData {
  name: string;
  username: string;
  password: string;
  credential_type: string;
  host: string;
  port: string;
  key_path: string;
  private_key: string;
  key_passphrase: string;
}
