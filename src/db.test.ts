import { describe, it, expect, vi } from 'vitest';
import { initDatabase } from './db';

// Mock the SQL plugin with a trackable execute function
const mockExecute = vi.fn().mockResolvedValue({ rowsAffected: 0 });
const mockSelect = vi.fn().mockResolvedValue([]);

vi.mock('@tauri-apps/plugin-sql', () => ({
  default: {
    load: vi.fn().mockResolvedValue({
      execute: (...args: any[]) => mockExecute(...args),
      select: (...args: any[]) => mockSelect(...args),
    }),
  },
}));

describe('Database Initialization', () => {
  it('should call execute with correct table schemas', async () => {
    await initDatabase();

    expect(mockExecute).toHaveBeenCalled();

    const queries = mockExecute.mock.calls.map((call: any) => call[0]);
    const hostsQuery = queries.find((q: string) => q.includes('CREATE TABLE IF NOT EXISTS hosts'));

    expect(hostsQuery).toBeDefined();
    expect(hostsQuery).toContain('id TEXT PRIMARY KEY');
    expect(hostsQuery).toContain('created_at INTEGER NOT NULL');
    expect(hostsQuery).toContain('updated_at INTEGER NOT NULL');
  });
});