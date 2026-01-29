import { describe, it, expect, vi } from 'vitest';
import { initDatabase } from './db';

// Mock the SQL plugin
vi.mock('@tauri-apps/plugin-sql', () => {
  const execute = vi.fn().mockResolvedValue({ rowsAffected: 0 });
  return {
    default: {
      load: vi.fn().mockResolvedValue({
        execute,
      }),
    },
    // Exporting the mock for verification if needed, 
    // but we can also get it from the mock instance
  };
});

describe('Database Initialization', () => {
  it('should call execute with correct table schemas', async () => {
    const db = await initDatabase();
    
    // Should be called twice (one for hosts, one for credentials)
    expect(db.execute).toHaveBeenCalledTimes(2);
    
    // @ts-ignore
    const hostsQuery = db.execute.mock.calls[0][0];
    expect(hostsQuery).toContain('CREATE TABLE IF NOT EXISTS hosts');
    expect(hostsQuery).toContain('protocol TEXT NOT NULL');

    // @ts-ignore
    const credsQuery = db.execute.mock.calls[1][0];
    expect(credsQuery).toContain('CREATE TABLE IF NOT EXISTS credentials');
    expect(credsQuery).toContain('password_encrypted BLOB NOT NULL');
  });
});