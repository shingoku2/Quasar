use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use chrono::Utc;
use crate::db;

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

    pub fn get_audit_logs(&self, filter: Option<AuditLogFilter>) -> Result<Vec<AuditLogEntry>, String> {
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

        let mut stmt = conn.prepare(&query)
            .map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let logs = stmt.query_map(params_refs.as_slice(), |row| {
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
        }).map_err(|e| format!("Failed to query audit logs: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect audit logs: {}", e))?;

        Ok(logs)
    }

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

        let count: i64 = conn.query_row(&query, params_refs.as_slice(), |row| row.get(0))
            .map_err(|e| format!("Failed to get audit log count: {}", e))?;

        Ok(count)
    }
}
