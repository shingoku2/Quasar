import Database from "@tauri-apps/plugin-sql";

/**
 * Opens a connection to the application SQLite database.
 *
 * Schema is managed exclusively by the Rust backend via rusqlite_migration
 * (see src-tauri/migrations/). This function must NOT create or alter tables.
 */
export async function initDatabase() {
  const db = await Database.load("sqlite:quasar.db");
  return db;
}
