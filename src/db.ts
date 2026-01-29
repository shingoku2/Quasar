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

  // Initialize Credentials table
  await db.execute(`
    CREATE TABLE IF NOT EXISTS credentials (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      host_id INTEGER,
      label TEXT NOT NULL,
      username TEXT NOT NULL,
      password_encrypted BLOB NOT NULL,
      iv BLOB NOT NULL,
      tag BLOB NOT NULL,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
      FOREIGN KEY(host_id) REFERENCES hosts(id) ON DELETE CASCADE
    );
  `);

  return db;
}
