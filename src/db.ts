import Database from "@tauri-apps/plugin-sql";

export async function initDatabase() {
  const db = await Database.load("sqlite:titan.db");

  // Initialize Hosts table
  await db.execute(`
    CREATE TABLE IF NOT EXISTS hosts (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      address TEXT NOT NULL,
      protocol TEXT NOT NULL,
      port INTEGER,
      username TEXT,
      last_connected DATETIME,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );
  `);

  return db;
}
