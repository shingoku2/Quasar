use crate::db;
use serde::{Deserialize, Serialize};

/// Appends one row to `security_audit_log`. Every audit write goes through here. A
/// `Transaction` derefs to `Connection`, so pass `&tx` to log inside a transaction.
pub(crate) fn insert_event(
    conn: &rusqlite::Connection,
    event_type: &str,
    resource_id: Option<&str>,
    resource_type: Option<&str>,
    action: &str,
    result: &str,
    details: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO security_audit_log (id, timestamp, event_type, resource_id, resource_type, action, result, details)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            chrono::Utc::now().timestamp(),
            event_type,
            resource_id,
            resource_type,
            action,
            result,
            details
        ],
    )
    .map(|_| ())
    .map_err(|e| format!("Failed to log audit event: {}", e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: i64,
    pub event_type: String,
    pub resource_id: Option<String>,
    pub resource_type: Option<String>,
    pub action: String,
    pub result: String,
    pub details: Option<String>,
    pub user_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogFilter {
    pub event_type: Option<String>,
    pub result: Option<String>,
    pub search_query: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub struct AuditLogManager {
    db_path: String,
}

impl AuditLogManager {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    /// Records a security event (best effort: failures are logged, never returned), for
    /// actions outside the vault module such as scheduled-task changes (IPC-011).
    pub fn record(
        &self,
        event_type: &str,
        resource_id: Option<&str>,
        resource_type: &str,
        action: &str,
        result: &str,
        details: Option<&str>,
    ) {
        let outcome = crate::db::open_connection(&self.db_path).and_then(|conn| {
            insert_event(&conn, event_type, resource_id, Some(resource_type), action, result, details)
        });
        if let Err(e) = outcome {
            log::warn!("Failed to record audit event {}: {}", event_type, e);
        }
    }

    pub fn get_audit_logs(
        &self,
        filter: Option<AuditLogFilter>,
    ) -> Result<Vec<AuditLogEntry>, String> {
        let conn = db::open_connection(&self.db_path)?;

        let filter = filter.unwrap_or(AuditLogFilter {
            event_type: None,
            result: None,
            search_query: None,
            limit: Some(100),
            offset: Some(0),
        });

        let mut query = String::from(
            "SELECT id, timestamp, event_type, resource_id, resource_type, action, result, details, user_context
             FROM security_audit_log WHERE 1=1"
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref event_type) = filter.event_type {
            query.push_str(" AND event_type = ?");
            params.push(Box::new(event_type.clone()));
        }

        if let Some(ref result) = filter.result {
            query.push_str(" AND result = ?");
            params.push(Box::new(result.clone()));
        }

        if let Some(ref search) = filter.search_query {
            query.push_str(" AND (event_type LIKE ? OR action LIKE ? OR details LIKE ?)");
            let search_pattern = format!("%{}%", search);
            params.push(Box::new(search_pattern.clone()));
            params.push(Box::new(search_pattern.clone()));
            params.push(Box::new(search_pattern));
        }

        query.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = filter.limit {
            query.push_str(" LIMIT ?");
            params.push(Box::new(limit));
        }

        if let Some(offset) = filter.offset {
            query.push_str(" OFFSET ?");
            params.push(Box::new(offset));
        }

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let logs = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(AuditLogEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    event_type: row.get(2)?,
                    resource_id: row.get(3)?,
                    resource_type: row.get(4)?,
                    action: row.get(5)?,
                    result: row.get(6)?,
                    details: row.get(7)?,
                    user_context: row.get(8)?,
                })
            })
            .map_err(|e| format!("Failed to query audit logs: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect audit logs: {}", e))?;

        Ok(logs)
    }

    #[cfg(test)]
    pub fn get_audit_log_count(&self, filter: Option<AuditLogFilter>) -> Result<i64, String> {
        let conn = db::open_connection(&self.db_path)?;

        let filter = filter.unwrap_or(AuditLogFilter {
            event_type: None,
            result: None,
            search_query: None,
            limit: None,
            offset: None,
        });

        let mut query = String::from("SELECT COUNT(*) FROM security_audit_log WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref event_type) = filter.event_type {
            query.push_str(" AND event_type = ?");
            params.push(Box::new(event_type.clone()));
        }

        if let Some(ref result) = filter.result {
            query.push_str(" AND result = ?");
            params.push(Box::new(result.clone()));
        }

        if let Some(ref search) = filter.search_query {
            query.push_str(" AND (event_type LIKE ? OR action LIKE ? OR details LIKE ?)");
            let search_pattern = format!("%{}%", search);
            params.push(Box::new(search_pattern.clone()));
            params.push(Box::new(search_pattern.clone()));
            params.push(Box::new(search_pattern));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&query, params_refs.as_slice(), |row| row.get(0))
            .map_err(|e| format!("Failed to get audit log count: {}", e))?;

        Ok(count)
    }

    /// Delete audit entries older than `retention_days`. Returns rows removed.
    ///
    /// The table has no bound otherwise: every credential read writes a row, and
    /// background host monitoring reads a credential per host on every poll, so
    /// an idle app still accumulates entries indefinitely. That inflates the
    /// database, slows the audit queries, and buries genuine access events under
    /// automated noise.
    pub fn cleanup_old_entries(&self, retention_days: u32) -> Result<usize, String> {
        let conn = db::open_connection(&self.db_path)?;
        let cutoff = chrono::Utc::now().timestamp() - (retention_days as i64 * 86_400);

        let deleted = conn
            .execute(
                "DELETE FROM security_audit_log WHERE timestamp < ?1",
                rusqlite::params![cutoff],
            )
            .map_err(|e| format!("Failed to prune audit log: {}", e))?;

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_conn(db_path: &str) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE security_audit_log (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                resource_id TEXT,
                resource_type TEXT,
                action TEXT NOT NULL,
                result TEXT NOT NULL,
                details TEXT,
                user_context TEXT
            );",
        )
        .unwrap();
        conn
    }

    fn insert_entry(conn: &rusqlite::Connection, id: &str, timestamp: i64) {
        conn.execute(
            "INSERT INTO security_audit_log (id, timestamp, event_type, action, result)
             VALUES (?1, ?2, 'credential_access', 'read', 'success')",
            rusqlite::params![id, timestamp],
        )
        .unwrap();
    }

    #[test]
    fn test_cleanup_removes_only_entries_past_retention() {
        let db_path = format!(
            "test_audit_cleanup_{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let conn = audit_conn(&db_path);

        let now = chrono::Utc::now().timestamp();
        insert_entry(&conn, "recent", now - 86_400); // 1 day old
        insert_entry(&conn, "edge", now - (89 * 86_400)); // just inside 90 days
        insert_entry(&conn, "stale", now - (120 * 86_400)); // well past retention
        drop(conn);

        let manager = AuditLogManager::new(db_path.clone());
        let deleted = manager.cleanup_old_entries(90).unwrap();
        assert_eq!(deleted, 1, "only the entry past retention should be removed");

        let remaining = manager.get_audit_log_count(None).unwrap();
        assert_eq!(remaining, 2, "entries inside the window must be preserved");

        let _ = std::fs::remove_file(&db_path);
    }

    /// A fresh audit table in its own file; removed when the guard drops.
    struct TempAuditDb(String);

    impl TempAuditDb {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "quasar_audit_{}_{}.db",
                tag,
                uuid::Uuid::new_v4()
            ));
            let path = path.to_string_lossy().into_owned();
            drop(audit_conn(&path));
            Self(path)
        }

        fn manager(&self) -> AuditLogManager {
            AuditLogManager::new(self.0.clone())
        }
    }

    impl Drop for TempAuditDb {
        fn drop(&mut self) {
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{}", self.0, suffix));
            }
        }
    }

    fn insert_full(
        path: &str,
        id: &str,
        timestamp: i64,
        event_type: &str,
        action: &str,
        result: &str,
        details: Option<&str>,
    ) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute(
            "INSERT INTO security_audit_log (id, timestamp, event_type, action, result, details)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, timestamp, event_type, action, result, details],
        )
        .unwrap();
    }

    fn filter() -> AuditLogFilter {
        AuditLogFilter {
            event_type: None,
            result: None,
            search_query: None,
            limit: None,
            offset: None,
        }
    }

    fn ids(entries: &[AuditLogEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.id.as_str()).collect()
    }

    fn seeded(tag: &str) -> TempAuditDb {
        let db = TempAuditDb::new(tag);
        insert_full(&db.0, "a", 100, "credential_access", "read", "success", Some("ssh to web01"));
        insert_full(&db.0, "b", 200, "credential_access", "reveal", "denied", None);
        insert_full(&db.0, "c", 300, "vault_unlock", "unlock", "failure", Some("bad password"));
        insert_full(&db.0, "d", 400, "scheduled_task", "update", "success", Some("nightly backup"));
        db
    }

    #[test]
    fn recorded_events_are_readable_with_every_field() {
        let db = TempAuditDb::new("record");
        let manager = db.manager();
        let before = chrono::Utc::now().timestamp();
        manager.record(
            "scheduled_task",
            Some("task-1"),
            "scheduled_task",
            "create",
            "success",
            Some("name=Nightly"),
        );
        manager.record("scheduled_task", None, "scheduled_task", "delete", "success", None);

        let logs = manager.get_audit_logs(None).unwrap();
        assert_eq!(logs.len(), 2);
        let created = logs.iter().find(|e| e.action == "create").unwrap();
        assert_eq!(created.event_type, "scheduled_task");
        assert_eq!(created.resource_id.as_deref(), Some("task-1"));
        assert_eq!(created.resource_type.as_deref(), Some("scheduled_task"));
        assert_eq!(created.result, "success");
        assert_eq!(created.details.as_deref(), Some("name=Nightly"));
        assert!(created.user_context.is_none());
        assert!(created.timestamp >= before);
        let deleted = logs.iter().find(|e| e.action == "delete").unwrap();
        assert!(deleted.resource_id.is_none());
        assert!(deleted.details.is_none());
        assert_ne!(created.id, deleted.id, "each event gets its own id");
    }

    #[test]
    fn recording_is_best_effort_when_the_table_is_missing() {
        let path = std::env::temp_dir()
            .join(format!("quasar_audit_missing_{}.db", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into_owned();
        let manager = AuditLogManager::new(path.clone());
        // Must not panic or return an error: auditing never fails the audited action.
        manager.record("x", None, "y", "z", "success", None);
        assert!(manager.get_audit_logs(None).is_err());
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path, suffix));
        }
    }

    #[test]
    fn logs_are_newest_first() {
        let db = seeded("order");
        let logs = db.manager().get_audit_logs(None).unwrap();
        assert_eq!(ids(&logs), vec!["d", "c", "b", "a"]);
    }

    #[test]
    fn default_filter_caps_the_result_at_100() {
        let db = TempAuditDb::new("cap");
        let conn = rusqlite::Connection::open(&db.0).unwrap();
        for i in 0..105 {
            conn.execute(
                "INSERT INTO security_audit_log (id, timestamp, event_type, action, result)
                 VALUES (?1, ?2, 'credential_access', 'read', 'success')",
                rusqlite::params![format!("e{i}"), i],
            )
            .unwrap();
        }
        drop(conn);
        let manager = db.manager();
        assert_eq!(manager.get_audit_logs(None).unwrap().len(), 100);
        // An explicit filter without a limit returns everything.
        assert_eq!(manager.get_audit_logs(Some(filter())).unwrap().len(), 105);
        assert_eq!(manager.get_audit_log_count(None).unwrap(), 105);
    }

    #[test]
    fn filters_by_event_type_and_result() {
        let db = seeded("filters");
        let manager = db.manager();
        let by_type = manager
            .get_audit_logs(Some(AuditLogFilter { event_type: Some("credential_access".into()), ..filter() }))
            .unwrap();
        assert_eq!(ids(&by_type), vec!["b", "a"]);

        let by_result = manager
            .get_audit_logs(Some(AuditLogFilter { result: Some("success".into()), ..filter() }))
            .unwrap();
        assert_eq!(ids(&by_result), vec!["d", "a"]);

        let both = AuditLogFilter {
            event_type: Some("credential_access".into()),
            result: Some("denied".into()),
            ..filter()
        };
        assert_eq!(ids(&manager.get_audit_logs(Some(both.clone())).unwrap()), vec!["b"]);
        assert_eq!(manager.get_audit_log_count(Some(both)).unwrap(), 1);
    }

    #[test]
    fn search_matches_event_type_action_or_details() {
        let db = seeded("search");
        let manager = db.manager();
        let search = |q: &str| {
            let f = AuditLogFilter { search_query: Some(q.into()), ..filter() };
            let found = manager.get_audit_logs(Some(f.clone())).unwrap();
            assert_eq!(manager.get_audit_log_count(Some(f)).unwrap() as usize, found.len());
            found.into_iter().map(|e| e.id).collect::<Vec<_>>()
        };
        assert_eq!(search("vault"), vec!["c"]); // event_type
        assert_eq!(search("reveal"), vec!["b"]); // action
        assert_eq!(search("web01"), vec!["a"]); // details
        assert_eq!(search("backup"), vec!["d"]);
        assert!(search("nothing-matches").is_empty());
    }

    #[test]
    fn search_text_is_bound_as_a_parameter_not_sql() {
        let db = seeded("inject");
        let manager = db.manager();
        let f = AuditLogFilter { search_query: Some("' OR 1=1 --".into()), ..filter() };
        assert!(manager.get_audit_logs(Some(f)).unwrap().is_empty());
        let f = AuditLogFilter {
            event_type: Some("x'; DROP TABLE security_audit_log; --".into()),
            ..filter()
        };
        assert!(manager.get_audit_logs(Some(f)).unwrap().is_empty());
        assert_eq!(manager.get_audit_log_count(None).unwrap(), 4, "the table survives");
    }

    #[test]
    fn limit_and_offset_page_through_newest_first() {
        let db = seeded("paging");
        let manager = db.manager();
        let page = |limit, offset| {
            let f = AuditLogFilter { limit: Some(limit), offset: Some(offset), ..filter() };
            manager.get_audit_logs(Some(f)).unwrap().into_iter().map(|e| e.id).collect::<Vec<_>>()
        };
        assert_eq!(page(2, 0), vec!["d", "c"]);
        assert_eq!(page(2, 2), vec!["b", "a"]);
        assert!(page(2, 4).is_empty());
    }

    #[test]
    fn cleanup_with_nothing_old_removes_nothing() {
        let db = TempAuditDb::new("cleanup_none");
        let manager = db.manager();
        manager.record("e", None, "t", "a", "success", None);
        assert_eq!(manager.cleanup_old_entries(1).unwrap(), 0);
        assert_eq!(manager.get_audit_log_count(None).unwrap(), 1);
    }
}
