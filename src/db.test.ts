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
    const queries = (Database.prototype.execute as any).mock.calls.map((call: any) => call[0]);
    
    // Should be called once for hosts
    expect(Database.prototype.execute).toHaveBeenCalledTimes(1);
    
    const hostsQuery = queries.find((q: string) => q.includes('CREATE TABLE IF NOT EXISTS hosts'));
    
    expect(hostsQuery).toBeDefined();
    expect(hostsQuery).toContain('id INTEGER PRIMARY KEY AUTOINCREMENT');
  });
});