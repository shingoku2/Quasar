import Database from "@tauri-apps/plugin-sql";

export async function initDatabase() {
<<<<<<< HEAD
  const db = await Database.load("sqlite:quasar.db");
=======
  const db = await Database.load("sqlite:titan.db");
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7

  // Initialize Hosts table
  await db.execute(`
    CREATE TABLE IF NOT EXISTS hosts (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      address TEXT NOT NULL,
      port INTEGER NOT NULL DEFAULT 22,
      username TEXT,
      protocol TEXT NOT NULL DEFAULT 'ssh',
      created_at INTEGER NOT NULL,
      updated_at INTEGER NOT NULL
    );
  `);

  return db;
}
