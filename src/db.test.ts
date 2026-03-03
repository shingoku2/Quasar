import { describe, it, expect, vi } from 'vitest';
import { initDatabase } from './db';

// Mock the SQL plugin
const mockDb = { execute: vi.fn(), select: vi.fn() };
const mockLoad = vi.fn().mockResolvedValue(mockDb);

vi.mock('@tauri-apps/plugin-sql', () => ({
  default: {
    load: (...args: unknown[]) => mockLoad(...args),
  },
}));

describe('Database Initialization', () => {
  it('should open the quasar.db database connection', async () => {
    const db = await initDatabase();

    expect(mockLoad).toHaveBeenCalledWith('sqlite:quasar.db');
    expect(db).toBe(mockDb);
  });

  it('should not execute any DDL (schema is managed by Rust migrations)', async () => {
    mockDb.execute.mockClear();
    await initDatabase();

    expect(mockDb.execute).not.toHaveBeenCalled();
  });
});
