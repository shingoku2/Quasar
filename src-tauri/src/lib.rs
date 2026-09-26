mod ai;
mod background;
mod commands;
mod saved_hosts;
mod crypto;
mod db;
mod local_paths;
mod native_confirm;
#[cfg(test)]
mod ssh_test_server;
mod discovery;
mod errors;
mod health;
mod host_tracker;
mod launcher;
mod monitoring;
mod scanner;
mod scheduler;
mod sftp;
mod ssh;
mod ssh_auth;
mod ssh_connect;
mod ssh_exec;
mod ssh_pool;
mod ssh_tunnel;
mod tailscale;
mod validation;
mod vault;

use errors::sanitize_error;
use log::error;
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use std::str::FromStr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use db::backup::{import_database_at, migrate_titan_db_to_quasar, set_quasar_application_id};
use db::migrations::MIGRATIONS;
use db::{app_db_connection, DB_FILENAME};
// The command modules share helpers with each other through `use crate::*` (for example
// `check_credential_binding`), and the tests below reach them through `use super::*`.
#[allow(unused_imports)]
use commands::{ai::*, app_data::*, files::*, hosts::*, monitoring::*, network::*, scheduler::*, ssh::*, vault::*};
use saved_hosts::*;
use validation::{
    validate_cidr, validate_credential_name, validate_hostname, validate_ip,
    validate_master_password, validate_path, validate_port, validate_username,
};



#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .setup(|app| {
            // Get database path
            let app_dir = app.path().app_data_dir().map_err(|e| {
                error!("Failed to get app data dir: {}", e);
                e
            })?;

            std::fs::create_dir_all(&app_dir).map_err(|e| {
                error!("Failed to create app data dir: {}", e);
                e
            })?;
            // Holds the vault DB and its .bak/WAL files: owner-only (RSEC-009).
            if let Err(e) = db::restrict_to_owner(&app_dir) {
                log::warn!("Could not restrict app data dir permissions: {}", e);
            }
            migrate_titan_db_to_quasar(&app_dir).map_err(|e| {
                error!("Failed to migrate database file: {}", e);
                e
            })?;

            let db_path = app_dir.join(DB_FILENAME);
            let db_path_str = db_path.to_str().ok_or("Invalid database path")?;
            let db_path_str = db_path_str.to_string();

            app.manage(ssh::SshState::new());
            app.manage(ssh::HostKeyApprovalState::new());
            app.manage(local_paths::LocalPathGrants::new());
            // Reuses authenticated sessions for one-shot commands (monitoring
            // probes, scheduled tasks) instead of re-handshaking every time.
            app.manage(ssh_pool::SshConnectionPool::new());
            app.manage(ssh_tunnel::TunnelState::new());
            app.manage(discovery::DiscoveryState::new());
            app.manage(Arc::new(scanner::ScannerState::new()));
            app.manage(Arc::new(monitoring::AlertEngine::new()));
            app.manage(vault::VaultState::new(db_path_str.clone()));
            app.manage(vault::CredentialManager::new(db_path_str.clone()));
            app.manage(vault::SshKeyManager::new(db_path_str.clone()).map_err(|e| {
                error!("Failed to create SSH key manager: {}", e);
                e
            })?);
            app.manage(vault::AuditLogManager::new(db_path_str.clone()));
            app.manage(host_tracker::HostTracker::new(db_path_str.clone()));
            app.manage(Arc::new(std::sync::Mutex::new(
                monitoring::MetricsCollector::new(),
            )));

            // Initialize database tables with foreign key constraints enabled
            let mut conn = db::open_connection(&db_path_str).map_err(|e| {
                error!("Failed to open database at {}: {}", db_path_str, e);
                e
            })?;

            // Run migrations using rusqlite_migration
            MIGRATIONS.to_latest(&mut conn).map_err(|e| {
                error!("Failed to run migrations: {}", e);
                e
            })?;
            set_quasar_application_id(&conn).map_err(|e| {
                error!("Failed to set Quasar database marker: {}", e);
                e
            })?;
            // Alert rules are persisted (RUST-002); load them now that the table exists.
            match app.state::<Arc<monitoring::AlertEngine>>().attach_store(db_path_str.clone()) {
                Ok(n) => log::info!("Loaded {} alert rule(s)", n),
                Err(e) => error!("Failed to load alert rules: {}", e),
            }

            // Start the background monitoring task using Tauri's async runtime
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                monitoring::start_monitoring_task(app_handle, 5).await;
            });

            // Remote hosts health is polled by the frontend (Monitoring tab) every 30s via get_remote_hosts_health

            // Background loops that need the managed state above (keep this order).
            background::spawn_auto_lock(app.handle());
            background::spawn_ssh_idle_reaper(app.handle());

            // Start cron-based scheduled task runner (SSH commands on saved hosts)
            scheduler::start_scheduler(app.handle().clone());

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    builder
        .invoke_handler(tauri::generate_handler![
            commands::ssh::connect_rdp,
            commands::network::get_tailscale_status,
            commands::network::start_discovery,
            commands::ai::check_ai_status,
            commands::ai::list_ai_models,
            commands::ai::send_ai_chat,
            commands::ssh::connect_ssh,
            ssh::write_ssh,
            ssh::resize_ssh,
            ssh::disconnect_ssh,
            commands::network::scan_network,
            commands::network::stop_scan,
            commands::network::get_scan_progress,
            commands::network::is_scanning,
            commands::network::get_discovered_hosts,
            commands::network::delete_discovered_host,
            commands::hosts::get_saved_hosts,
            commands::hosts::upsert_saved_host,
            commands::hosts::update_saved_host,
            commands::hosts::remove_saved_hosts,
            commands::hosts::get_remote_hosts_health,
            commands::hosts::set_host_monitoring_credential,
            commands::network::preflight_check,
            commands::network::check_host_health,
            commands::monitoring::get_system_metrics,
            commands::monitoring::add_alert_rule,
            commands::monitoring::remove_alert_rule,
            commands::monitoring::get_alert_rules,
            commands::vault::is_vault_initialized,
            commands::vault::initialize_vault,
            commands::vault::unlock_vault,
            commands::vault::lock_vault,
            commands::vault::is_vault_locked,
            commands::vault::get_vault_settings,
            commands::vault::update_vault_settings,
            commands::vault::change_master_password,
            commands::vault::add_credential,
            commands::vault::get_credential,
            commands::vault::reveal_credential_password,
            commands::vault::list_credentials,
            commands::vault::update_credential,
            commands::vault::delete_credential,
            commands::vault::search_credentials,
            commands::vault::trust_ssh_host_key,
            commands::vault::respond_ssh_host_key_verification,
            commands::vault::get_known_ssh_hosts,
            commands::vault::remove_ssh_host_key,
            commands::vault::update_ssh_host_trust,
            commands::vault::get_audit_logs,
            commands::files::pick_local_file,
            commands::files::pick_save_location,
            commands::files::sftp_upload_file,
            commands::files::sftp_download_file,
            commands::files::sftp_list_directory,
            commands::scheduler::list_scheduled_tasks,
            commands::scheduler::add_scheduled_task,
            commands::scheduler::update_scheduled_task,
            commands::scheduler::remove_scheduled_task,
            commands::scheduler::run_scheduled_task_now,
            commands::ssh::start_ssh_tunnel,
            commands::ssh::list_ssh_tunnels,
            commands::ssh::close_ssh_tunnel,
            commands::app_data::get_app_info,
            commands::app_data::clear_metrics_data,
            commands::app_data::export_database,
            commands::app_data::import_database
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod saved_host_tests {
    use super::*;
    use crate::db::backup::{is_recognized_quasar_database, restore_previous_database};
    use crate::db::migrations::LATEST_SCHEMA_VERSION;

    fn host_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE hosts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                address TEXT NOT NULL,
                port INTEGER NOT NULL DEFAULT 22,
                username TEXT,
                protocol TEXT NOT NULL DEFAULT 'ssh',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE monitoring_host_credential (
                host_id TEXT PRIMARY KEY,
                credential_id TEXT
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn upsert_saved_host_inserts_with_default_ssh_port() {
        let mut conn = host_conn();
        let host = upsert_saved_host_in_conn(
            &mut conn,
            "Server",
            "server.example.com",
            "ssh",
            None,
            Some("root"),
        )
        .unwrap();

        assert_eq!(host.port, 22);
        assert_eq!(host.username.as_deref(), Some("root"));
        assert!(!host.id.is_empty());
    }

    #[test]
    fn upsert_saved_host_updates_matching_connection() {
        let mut conn = host_conn();
        let original =
            upsert_saved_host_in_conn(&mut conn, "Old name", "10.0.0.5", "ssh", Some(2222), None)
                .unwrap();
        let updated = upsert_saved_host_in_conn(
            &mut conn,
            "New name",
            "10.0.0.5",
            "ssh",
            Some(2222),
            Some("admin"),
        )
        .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.name, "New name");
        assert_eq!(updated.username.as_deref(), Some("admin"));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM hosts", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn update_saved_host_changes_connection_port_in_place() {
        let mut conn = host_conn();
        let original = upsert_saved_host_in_conn(
            &mut conn,
            "VPS",
            "15.204.11.162",
            "ssh",
            Some(22),
            Some("edward"),
        )
        .unwrap();

        let updated = update_saved_host_in_conn(
            &conn,
            &original.id,
            "VPS",
            "15.204.11.162",
            "ssh",
            Some(6969),
            Some("edward"),
        )
        .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.port, 6969);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM hosts", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn upsert_saved_host_uses_default_rdp_port() {
        let mut conn = host_conn();
        let host = upsert_saved_host_in_conn(
            &mut conn,
            "Desktop",
            "desktop.example.com",
            "rdp",
            None,
            None,
        )
        .unwrap();

        assert_eq!(host.port, 3389);
    }

    #[test]
    fn remove_saved_hosts_is_transactional_and_ignores_missing_ids() {
        let mut conn = host_conn();
        let first = upsert_saved_host_in_conn(&mut conn, "One", "one", "ssh", None, None).unwrap();
        let second = upsert_saved_host_in_conn(&mut conn, "Two", "two", "ssh", None, None).unwrap();

        let removed =
            remove_saved_hosts_in_conn(&mut conn, &[first.id, "missing".to_string(), second.id])
                .unwrap();

        assert_eq!(removed, 2);
        assert!(get_saved_hosts_from_conn(&conn).unwrap().is_empty());
    }

    #[test]
    fn remove_saved_hosts_removes_one_host() {
        let mut conn = host_conn();
        let host = upsert_saved_host_in_conn(&mut conn, "One", "one", "ssh", None, None).unwrap();

        assert_eq!(
            remove_saved_hosts_in_conn(&mut conn, std::slice::from_ref(&host.id)).unwrap(),
            1
        );
        assert!(get_saved_host_by_id(&conn, &host.id).unwrap().is_none());
    }

    #[test]
    fn remove_saved_hosts_rolls_back_on_error() {
        let mut conn = host_conn();
        let first = upsert_saved_host_in_conn(&mut conn, "One", "one", "ssh", None, None).unwrap();
        let second = upsert_saved_host_in_conn(&mut conn, "Two", "two", "ssh", None, None).unwrap();
        conn.execute_batch(&format!(
            "CREATE TRIGGER reject_host_delete BEFORE DELETE ON hosts
             WHEN OLD.id = '{}' BEGIN SELECT RAISE(ABORT, 'stop'); END;",
            second.id
        ))
        .unwrap();

        assert!(remove_saved_hosts_in_conn(&mut conn, &[first.id, second.id]).is_err());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM hosts", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn saved_host_operations_return_database_errors() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        assert!(upsert_saved_host_in_conn(&mut conn, "Host", "host", "ssh", None, None).is_err());
        assert!(remove_saved_hosts_in_conn(&mut conn, &["id".to_string()]).is_err());
    }

    fn migrated_quasar_db(path: &std::path::Path) {
        let mut conn = rusqlite::Connection::open(path).unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        set_quasar_application_id(&conn).unwrap();
    }

    fn import_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("quasar_import_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// RSEC-004: importing replaces the vault's salt/verifier, so the vault must end up
    /// locked (its in-memory key belongs to the old database).
    #[tokio::test]
    async fn import_locks_the_vault() {
        let dir = import_test_dir("lock");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        vault
            .initialize_vault(secrecy::SecretString::from("LivePassword123!"))
            .await
            .unwrap();
        assert!(!vault.is_locked().await);

        let source = dir.join("backup.db");
        migrated_quasar_db(&source);
        import_database_at(&dir, source.to_str().unwrap(), &vault, None).await.unwrap();

        assert!(vault.is_locked().await, "vault must be locked after an import");
        assert!(dir.join(format!("{}.bak", DB_FILENAME)).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RUST-004: a backup from a newer schema is rejected instead of bricking startup, and
    /// the live database and vault are left alone.
    #[tokio::test]
    async fn import_rejects_newer_schema_and_leaves_vault_alone() {
        let dir = import_test_dir("newer");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        vault
            .initialize_vault(secrecy::SecretString::from("LivePassword123!"))
            .await
            .unwrap();

        let source = dir.join("future.db");
        migrated_quasar_db(&source);
        rusqlite::Connection::open(&source)
            .unwrap()
            .pragma_update(None, "user_version", LATEST_SCHEMA_VERSION + 1)
            .unwrap();
        let err = import_database_at(&dir, source.to_str().unwrap(), &vault, None)
            .await
            .unwrap_err();
        assert!(err.contains("newer version"), "{}", err);
        assert!(!vault.is_locked().await);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: a backup from an older schema is migrated as part of the import, so the
    /// running app finds every current table (startup migrations don't run again).
    #[tokio::test]
    async fn import_migrates_an_older_backup() {
        let dir = import_test_dir("older");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());

        let source = dir.join("old.db");
        {
            let mut conn = rusqlite::Connection::open(&source).unwrap();
            MIGRATIONS.to_version(&mut conn, (LATEST_SCHEMA_VERSION - 1) as usize).unwrap();
            set_quasar_application_id(&conn).unwrap();
            let has_rules: bool = conn
                .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'alert_rules')", [], |r| r.get(0))
                .unwrap();
            assert!(!has_rules, "fixture: schema {} predates alert_rules", LATEST_SCHEMA_VERSION - 1);
        }
        import_database_at(&dir, source.to_str().unwrap(), &vault, None).await.unwrap();

        let conn = rusqlite::Connection::open(&live).unwrap();
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, LATEST_SCHEMA_VERSION);
        conn.execute("INSERT INTO alert_rules (id, rule, updated_at) VALUES ('r', '{}', 0)", [])
            .expect("alert_rules exists after importing an older backup");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: the alert engine follows an imported database. Its rules were loaded
    /// once at startup, so monitoring kept evaluating the pre-import rules until restart.
    #[tokio::test]
    async fn import_reloads_alert_rules() {
        let dir = import_test_dir("alerts");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let add_rule = |path: &std::path::Path, id: &str| {
            let rule = monitoring::AlertRule {
                id: id.to_string(),
                metric: monitoring::MetricType::CpuUsage,
                operator: monitoring::ComparisonOperator::GreaterThan,
                threshold: 90.0,
                severity: monitoring::AlertSeverity::Warning,
                enabled: true,
                cooldown_seconds: 60,
            };
            rusqlite::Connection::open(path).unwrap()
                .execute(
                    "INSERT INTO alert_rules (id, rule, updated_at) VALUES (?1, ?2, 0)",
                    rusqlite::params![id, serde_json::to_string(&rule).unwrap()],
                )
                .unwrap();
        };
        add_rule(&live, "old-rule");
        let engine = monitoring::AlertEngine::new();
        assert_eq!(engine.attach_store(live.to_str().unwrap().to_string()).unwrap(), 1);

        let source = dir.join("backup.db");
        migrated_quasar_db(&source);
        add_rule(&source, "new-rule");
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        import_database_at(&dir, source.to_str().unwrap(), &vault, Some(&engine)).await.unwrap();

        let ids: Vec<String> = engine.get_rules().into_iter().map(|r| r.id).collect();
        assert_eq!(ids, vec!["new-rule".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: if the migration of an imported backup fails, the previous database is
    /// put back, and the error says so.
    #[tokio::test]
    async fn import_restores_the_previous_database_when_migration_fails() {
        let dir = import_test_dir("migfail");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        rusqlite::Connection::open(&live)
            .unwrap()
            .execute("INSERT INTO alert_rules (id, rule, updated_at) VALUES ('live-marker', '{}', 0)", [])
            .unwrap();
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());

        // A current schema claiming version 6: migration 008 re-adds a column that exists.
        let source = dir.join("broken.db");
        migrated_quasar_db(&source);
        rusqlite::Connection::open(&source).unwrap().pragma_update(None, "user_version", 6).unwrap();
        let err = import_database_at(&dir, source.to_str().unwrap(), &vault, None).await.unwrap_err();
        assert!(!err.contains("could not be restored"), "{}", err);

        let conn = rusqlite::Connection::open(&live).unwrap();
        let marker: i64 = conn
            .query_row("SELECT count(*) FROM alert_rules WHERE id = 'live-marker'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(marker, 1, "the previous database is live again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: the alert rules are suspended for the replacement and reloaded whatever
    /// the outcome, so a failed (and rolled back) import leaves the old rules active.
    #[tokio::test]
    async fn failed_import_keeps_the_previous_alert_rules() {
        let dir = import_test_dir("migfail_rules");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let engine = monitoring::AlertEngine::new();
        engine.attach_store(live.to_str().unwrap().to_string()).unwrap();
        engine
            .add_rule(monitoring::AlertRule {
                id: "live-rule".to_string(),
                metric: monitoring::MetricType::CpuUsage,
                operator: monitoring::ComparisonOperator::GreaterThan,
                threshold: 90.0,
                severity: monitoring::AlertSeverity::Warning,
                enabled: true,
                cooldown_seconds: 60,
            })
            .unwrap();
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());

        let source = dir.join("broken.db");
        migrated_quasar_db(&source);
        rusqlite::Connection::open(&source).unwrap().pragma_update(None, "user_version", 6).unwrap();
        import_database_at(&dir, source.to_str().unwrap(), &vault, Some(&engine)).await.unwrap_err();

        let ids: Vec<String> = engine.get_rules().into_iter().map(|r| r.id).collect();
        assert_eq!(ids, vec!["live-rule".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: the old vault's unlock lockout must not block the imported vault.
    #[tokio::test]
    async fn import_forgets_the_previous_vaults_lockout() {
        let dir = import_test_dir("lockout");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        vault.initialize_vault(secrecy::SecretString::from("LivePassword123!")).await.unwrap();
        vault.lock_vault().await.unwrap();
        for _ in 0..5 {
            let _ = vault.unlock_vault(secrecy::SecretString::from("wrong-password")).await;
        }
        let err = vault.unlock_vault(secrecy::SecretString::from("LivePassword123!")).await.unwrap_err();
        assert!(err.contains("locked out"), "fixture: the live vault is locked out: {}", err);

        let source = dir.join("other.db");
        migrated_quasar_db(&source);
        let other = vault::VaultState::new(source.to_str().unwrap().to_string());
        other.initialize_vault(secrecy::SecretString::from("OtherPassword123!")).await.unwrap();
        drop(other);
        import_database_at(&dir, source.to_str().unwrap(), &vault, None).await.unwrap();

        vault
            .unlock_vault(secrecy::SecretString::from("OtherPassword123!"))
            .await
            .expect("the imported vault unlocks with its own password");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: a restore that fails is reported, not swallowed.
    #[test]
    fn restore_previous_database_reports_failure() {
        let dir = import_test_dir("restorefail");
        let backup = dir.join("quasar.db.bak");
        std::fs::write(&backup, b"not a sqlite database, just some bytes long enough to have a header").unwrap();
        let mut dst = rusqlite::Connection::open(dir.join(DB_FILENAME)).unwrap();
        assert!(restore_previous_database(&backup, &mut dst).is_err());
        assert!(restore_previous_database(&dir.join("missing.bak"), &mut dst).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: when the imported alert rules can't be read, the import says so and
    /// monitoring runs with no rules, never the pre-import ones.
    #[tokio::test]
    async fn import_reports_unreadable_alert_rules_and_drops_the_old_ones() {
        let dir = import_test_dir("alertfail");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let rule = monitoring::AlertRule {
            id: "old-rule".to_string(),
            metric: monitoring::MetricType::CpuUsage,
            operator: monitoring::ComparisonOperator::GreaterThan,
            threshold: 90.0,
            severity: monitoring::AlertSeverity::Warning,
            enabled: true,
            cooldown_seconds: 60,
        };
        let engine = monitoring::AlertEngine::new();
        engine.attach_store(live.to_str().unwrap().to_string()).unwrap();
        engine.add_rule(rule).unwrap();

        // A rule row whose column isn't text can't be loaded.
        let source = dir.join("backup.db");
        migrated_quasar_db(&source);
        rusqlite::Connection::open(&source)
            .unwrap()
            .execute("INSERT INTO alert_rules (id, rule, updated_at) VALUES ('bad', x'00ff', 0)", [])
            .unwrap();
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        let err = import_database_at(&dir, source.to_str().unwrap(), &vault, Some(&engine))
            .await
            .unwrap_err();
        assert!(err.contains("alert rules could not be loaded"), "{}", err);
        assert!(engine.get_rules().is_empty(), "pre-import rules must not stay active");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: a local-path grant is single-use even when the transfer fails, so only
    /// the picker commands may grant a path; nothing hands a used grant back. Scans the
    /// production code of every source file, so it keeps holding as code moves.
    #[test]
    fn only_the_pickers_grant_local_paths() {
        fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().map(Result::unwrap) {
                let path = entry.path();
                if path.is_dir() {
                    rust_files(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_files(&src, &mut files);
        let mut grants = Vec::new();
        for file in files {
            let rel = file.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
            // local_paths.rs defines grant(); the test server is test-only.
            if rel == "local_paths.rs" || rel == "ssh_test_server.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&file).unwrap();
            // Production code ends at the first inline test module (`#[cfg(test)] mod x {`),
            // not at a `#[cfg(test)] mod x;` declaration.
            let cut = text
                .match_indices("#[cfg(test)]\nmod ")
                .map(|(i, _)| i)
                .find(|&i| text[i..].lines().nth(1).is_some_and(|l| l.trim_end().ends_with('{')))
                .unwrap_or(text.len());
            let production = &text[..cut];
            for line in production.lines().filter(|l| l.contains(".grant(")) {
                grants.push(format!("{}: {}", rel, line.trim()));
            }
        }
        assert_eq!(grants, vec!["commands/files.rs: grants.grant(path, intent);"], "unexpected grant call sites");
    }

    fn migrated_memory_db() -> rusqlite::Connection {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        conn
    }

    /// PR #68 review: moving a host that scheduled file transfers use needs a native
    /// confirmation; renaming it, or moving a host no transfer uses, doesn't.
    #[test]
    fn moving_a_host_with_scheduled_transfers_is_detected() {
        let conn = migrated_memory_db();
        conn.execute(
            "INSERT INTO hosts (id, name, address, protocol, port, created_at, updated_at) VALUES ('h1', 'box', 'db1.example', 'ssh', 22, 0, 0)",
            [],
        )
        .unwrap();
        let moved = |address: &str, port: Option<u16>| transfers_moved_by_host_edit(&conn, "h1", address, "ssh", port).unwrap();
        assert_eq!(moved("evil.example", Some(22)), None, "no transfer uses the host yet");
        conn.execute(
            "INSERT INTO scheduled_tasks (id, name, cron_expression, host_id, command, enabled, created_at, updated_at, task_type, local_path, remote_path)
             VALUES ('t1', 'up', '0 0 * * * *', 'h1', '', 1, 0, 0, 'sftp_upload', '/home/u/report.txt', '/r')",
            [],
        )
        .unwrap();
        assert_eq!(moved("DB1.example ", None), None, "same destination (case, default port)");
        assert_eq!(moved("evil.example", Some(22)), Some(("db1.example".to_string(), 22, 1)));
        assert_eq!(moved("db1.example", Some(2222)), Some(("db1.example".to_string(), 22, 1)));
        assert_eq!(transfers_moved_by_host_edit(&conn, "missing", "x", "ssh", None).unwrap(), None);
    }

    /// IPC-011: task creation rejects unknown types (which used to run as SSH exec),
    /// missing hosts, empty SSH commands and SFTP tasks without both paths.
    /// P7-3 review: an upload task's read grant must not carry over when the task is
    /// switched to a download (write) on the same path.
    #[test]
    fn scheduled_task_path_exemption_requires_same_type() {
        use local_paths::Intent;
        assert_eq!(task_local_intent("sftp_upload"), Some(Intent::Read));
        assert_eq!(task_local_intent("sftp_download"), Some(Intent::Write));
        assert_eq!(task_local_intent("ssh"), None);
        let stored = |p: &str, t: &str| Some((Some(p.to_string()), Some(t.to_string()), "h1".to_string(), Some("/r".to_string())));
        let same = |s| task_transfer_unchanged(s, "/a", "sftp_upload", "h1", Some("/r"));
        assert!(same(stored("/a", "sftp_upload")));
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/a", "sftp_download", "h1", Some("/r")));
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/b", "sftp_upload", "h1", Some("/r")));
        assert!(!same(None));
        assert!(!same(Some((None, Some("ssh".into()), "h1".into(), None))));
        // PR #68 review: a new destination (host or remote path) needs a new pick too.
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/a", "sftp_upload", "evil", Some("/r")));
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/a", "sftp_upload", "h1", Some("/tmp/x")));
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/a", "sftp_upload", "h1", None));
    }

    #[test]
    fn scheduled_task_validation() {
        let mut conn = migrated_memory_db();
        let host = upsert_saved_host_in_conn(&mut conn, "srv", "10.0.0.5", "ssh", None, Some("root")).unwrap();
        let ok = |t: Option<&str>, cmd: &str, lp: Option<&str>, rp: Option<&str>| {
            validate_scheduled_task(&conn, "backup", &host.id, cmd, t, lp, rp, None).map(str::to_string)
        };
        assert_eq!(ok(None, "uptime", None, None).unwrap(), "ssh");
        assert!(ok(Some("SFTP_UPLOAD"), "uptime", None, None).unwrap_err().contains("Unknown task type"));
        assert!(ok(Some("ssh"), "   ", None, None).is_err());
        assert!(ok(Some("sftp_upload"), "", Some("/tmp/a"), None).is_err());
        assert_eq!(ok(Some("sftp_upload"), "", Some("/tmp/a"), Some("/srv/a")).unwrap(), "sftp_upload");
        assert!(validate_scheduled_task(&conn, "t", "no-such-host", "uptime", None, None, None, None)
            .unwrap_err()
            .contains("Host not found"));
        assert!(validate_scheduled_task(&conn, "", &host.id, "uptime", None, None, None, None).is_err());
        // PR #68 review: inventory-only hosts (database, API, other) can't run tasks.
        for protocol in ["database", "api", "other", "rdp"] {
            let other = upsert_saved_host_in_conn(&mut conn, protocol, "10.0.0.9", protocol, Some(5432), None).unwrap();
            assert!(validate_scheduled_task(&conn, "t", &other.id, "uptime", None, None, None, None)
                .unwrap_err()
                .contains("SSH host"));
        }
        // A credential bound to another host is refused when the task is saved.
        conn.execute(
            "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, host, created_at, updated_at)
             VALUES ('elsewhere', 'elsewhere', 'u', X'00', zeroblob(12), zeroblob(16), 'password', 'evil.example', 0, 0)",
            [],
        )
        .unwrap();
        assert!(validate_scheduled_task(&conn, "t", &host.id, "uptime", None, None, None, Some("elsewhere"))
            .unwrap_err()
            .contains("restricted"));
    }

    /// PR #68 review: only SSH-type credentials authenticate SSH, and SFTP takes SSH
    /// passwords only. An API/database/RDP/other credential's secret is never sent to an SSH
    /// server; the check runs when a task or monitoring binding is saved and again at use.
    #[test]
    fn credential_type_must_match_its_use() {
        use vault::credentials::{type_allowed, CredentialUse::{Sftp, Ssh}};
        for t in ["ssh", "password"] {
            assert!(type_allowed(t, Ssh) && type_allowed(t, Sftp), "{}", t);
        }
        assert!(type_allowed("ssh_key", Ssh) && !type_allowed("ssh_key", Sftp));
        for t in ["api", "database", "rdp", "other", ""] {
            assert!(!type_allowed(t, Ssh) && !type_allowed(t, Sftp), "{}", t);
        }

        let mut conn = migrated_memory_db();
        let host = upsert_saved_host_in_conn(&mut conn, "srv", "10.0.0.5", "ssh", None, Some("root")).unwrap();
        for (id, ty) in [("api", "api"), ("key", "ssh_key"), ("pw", "ssh")] {
            conn.execute(
                "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, created_at, updated_at)
                 VALUES (?1, ?1, 'u', X'00', zeroblob(12), zeroblob(16), ?2, 0, 0)",
                rusqlite::params![id, ty],
            )
            .unwrap();
        }
        let (up, down) = (Some("sftp_upload"), Some("sftp_download"));
        let paths = (Some("/tmp/a"), Some("/tmp/b"));
        // SSH command task: password or key, never an API credential.
        assert!(validate_scheduled_task(&conn, "t", &host.id, "uptime", None, None, None, Some("pw")).is_ok());
        assert!(validate_scheduled_task(&conn, "t", &host.id, "uptime", None, None, None, Some("key")).is_ok());
        assert!(validate_scheduled_task(&conn, "t", &host.id, "uptime", None, None, None, Some("api"))
            .unwrap_err()
            .contains("SSH needs"));
        // SFTP task: SSH password only.
        assert!(validate_scheduled_task(&conn, "t", &host.id, "", up, paths.0, paths.1, Some("pw")).is_ok());
        for bad in ["key", "api"] {
            assert!(validate_scheduled_task(&conn, "t", &host.id, "", down, paths.0, paths.1, Some(bad))
                .unwrap_err()
                .contains("SFTP needs"));
        }
        // Monitoring binding.
        assert!(check_credential_binding(&conn, &host.id, "key", vault::credentials::CredentialUse::Ssh).is_ok());
        assert!(check_credential_binding(&conn, &host.id, "api", vault::credentials::CredentialUse::Ssh).is_err());
    }

    /// RUST-020: the one credential-to-login path for the terminal and tunnels keeps both
    /// checks: host binding and SSH-only types (invariant 10).
    #[tokio::test]
    async fn ssh_login_resolution_enforces_host_and_type() {
        let dir = import_test_dir("sshlogin");
        let db = dir.join(DB_FILENAME);
        migrated_quasar_db(&db);
        let db = db.to_str().unwrap().to_string();
        let vault = vault::VaultState::new(db.clone());
        vault.initialize_vault(secrecy::SecretString::from("LivePassword123!")).await.unwrap();
        let creds = vault::CredentialManager::new(db);
        let add = |name: &str, ty: &str, password: &str, host: Option<&str>| {
            let vault = &vault;
            let creds = &creds;
            let (name, ty, password, host) = (name.to_string(), ty.to_string(), password.to_string(), host.map(str::to_string));
            async move {
                let access = vault.credential_access().await.unwrap();
                creds
                    .add_credential(access.key(), name, "admin".into(), password, ty, host, None, None, None, None, None)
                    .unwrap()
            }
        };
        let bound = add("bound", "ssh", "s3cret", Some("db1")).await;
        let key_only = add("key", "ssh_key", "", None).await;
        let api = add("api", "api", "token", None).await;

        let typed = resolve_ssh_login(&vault, &creds, None, "any", "me".into(), Some("pw".into())).await.unwrap();
        assert_eq!((typed.username.as_str(), typed.password.as_deref()), ("me", Some("pw")));

        let ok = resolve_ssh_login(&vault, &creds, Some(bound.clone()), "DB1", "ignored".into(), None).await.unwrap();
        assert_eq!((ok.username.as_str(), ok.password.as_deref()), ("admin", Some("s3cret")));
        assert!(resolve_ssh_login(&vault, &creds, Some(bound), "evil.example", "x".into(), None).await.is_err());

        let key = resolve_ssh_login(&vault, &creds, Some(key_only), "any", "x".into(), None).await.unwrap();
        assert_eq!(key.password, None, "an empty stored password means none");

        let err = resolve_ssh_login(&vault, &creds, Some(api), "any", "x".into(), None).await.err().unwrap();
        assert!(err.contains("SSH needs"), "{}", err);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// IPC-003: a monitoring binding needs an existing host and credential, and a credential
    /// restricted to another host can't be bound.
    #[test]
    fn monitoring_binding_respects_credential_host() {
        let mut conn = migrated_memory_db();
        let host = upsert_saved_host_in_conn(&mut conn, "srv", "10.0.0.5", "ssh", None, Some("root")).unwrap();
        let insert = |id: &str, bound: Option<&str>| {
            conn.execute(
                "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, host, created_at, updated_at)
                 VALUES (?1, ?1, 'u', X'00', zeroblob(12), zeroblob(16), 'password', ?2, 0, 0)",
                rusqlite::params![id, bound],
            )
            .unwrap();
        };
        insert("free", None);
        insert("bound-here", Some("10.0.0.5"));
        insert("bound-elsewhere", Some("evil.example"));
        assert!(check_credential_binding(&conn, &host.id, "free", vault::credentials::CredentialUse::Ssh).is_ok());
        assert!(check_credential_binding(&conn, &host.id, "bound-here", vault::credentials::CredentialUse::Ssh).is_ok());
        assert!(check_credential_binding(&conn, &host.id, "bound-elsewhere", vault::credentials::CredentialUse::Ssh).unwrap_err().contains("restricted"));
        assert!(check_credential_binding(&conn, &host.id, "missing", vault::credentials::CredentialUse::Ssh).is_err());
        assert!(check_credential_binding(&conn, "no-host", "free", vault::credentials::CredentialUse::Ssh).is_err());
    }

    /// IPC-007 / IPC-008: the shipped capability and CSP stay least-privilege.
    #[test]
    fn capability_and_csp_stay_least_privilege() {
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let perms: Vec<String> = cap["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().map(str::to_string).unwrap_or_else(|| p["identifier"].as_str().unwrap().to_string()))
            .collect();
        for banned in ["core:default", "core:event:default", "core:event:allow-emit", "core:event:allow-emit-to", "opener:default", "opener:allow-reveal-item-in-dir", "dialog:default"] {
            assert!(!perms.iter().any(|p| p == banned), "capability must not grant {}", banned);
        }

        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let csp = conf["app"]["security"]["csp"].as_str().unwrap();
        assert!(!csp.contains("localhost:*"), "production CSP must not allow arbitrary localhost ports: {}", csp);
        assert!(csp.contains("ipc:"), "production CSP must allow Tauri IPC: {}", csp);
    }

    #[test]
    fn migrations_apply_to_fresh_and_current_databases() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let version = conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(version, 14);
        assert_eq!(version, LATEST_SCHEMA_VERSION, "update LATEST_SCHEMA_VERSION with MIGRATIONS");

        MIGRATIONS.to_latest(&mut conn).unwrap();
        let version_after_rerun = conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(version_after_rerun, version);
    }

    /// CLEAN-002: docs/ARCHITECTURE.md must list every registered command (CLAUDE.md once
    /// listed 21 that didn't exist). Reads `generate_handler!` from this file's source.
    #[test]
    fn architecture_doc_lists_every_command() {
        let source = include_str!("lib.rs");
        let doc = include_str!("../../docs/ARCHITECTURE.md");
        let start = source.find(concat!("generate_handler", "![")).expect("handler list") + 18;
        let list = &source[start..start + source[start..].find(']').unwrap()];
        let commands: Vec<&str> = list
            .split(',')
            .map(|c| c.trim().rsplit("::").next().unwrap_or(""))
            .filter(|c| !c.is_empty())
            .collect();
        assert!(commands.len() > 50, "parsed {} commands", commands.len());
        let missing: Vec<&str> = commands
            .into_iter()
            .filter(|c| !doc.contains(&format!("| `{}` |", c)))
            .collect();
        assert!(missing.is_empty(), "docs/ARCHITECTURE.md is missing commands: {:?}", missing);
    }

    /// RUST-018: docs/SCHEMA.md must describe every table and column the migrations create
    /// (a `### `table`` heading and a `| `column` |` row each), so it can't drift again.
    /// On failure it prints the tables as they should be documented.
    #[test]
    fn schema_doc_lists_every_table_and_column() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let doc = include_str!("../../docs/SCHEMA.md");
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let mut missing = Vec::new();
        let mut expected = String::new();
        for table in &tables {
            let heading = format!("### `{}`", table);
            if !doc.contains(&heading) {
                missing.push(heading.clone());
            }
            expected.push_str(&format!("\n{}\n\n| Column | Type | Null | Default | Key |\n|---|---|---|---|---|\n", heading));
            let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{}\")", table)).unwrap();
            let cols = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, bool>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?,
                    ))
                })
                .unwrap();
            for col in cols {
                let (name, ty, notnull, default, pk) = col.unwrap();
                let row = format!("| `{}` |", name);
                if !doc.contains(&heading) || !doc[doc.find(&heading).unwrap_or(0)..].contains(&row) {
                    missing.push(format!("{}.{}", table, name));
                }
                expected.push_str(&format!(
                    "{} {} | {} | {} | {} |\n",
                    row,
                    ty,
                    if notnull { "NOT NULL" } else { "" },
                    default.unwrap_or_default(),
                    if pk > 0 { "PK" } else { "" }
                ));
            }
        }
        assert!(missing.is_empty(), "docs/SCHEMA.md is missing {:?}. Current schema:\n{}", missing, expected);
    }

    #[test]
    fn database_recognition_accepts_quasar_application_id() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        set_quasar_application_id(&conn).unwrap();
        assert!(is_recognized_quasar_database(&conn).unwrap());
    }

    #[test]
    fn database_recognition_accepts_strict_legacy_schema() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE hosts (id TEXT, name TEXT, address TEXT, port INTEGER);
            CREATE TABLE credentials (id TEXT, name TEXT, username TEXT);
            CREATE TABLE vault_settings (key TEXT, value TEXT);
            CREATE TABLE security_audit_log (id TEXT, event_type TEXT, action TEXT);
            CREATE TABLE ssh_known_hosts (id TEXT, host TEXT, fingerprint TEXT);
            ",
        )
        .unwrap();
        assert!(is_recognized_quasar_database(&conn).unwrap());
    }

    #[test]
    fn database_recognition_rejects_unrelated_sqlite() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE random_table (id INTEGER);")
            .unwrap();
        assert!(!is_recognized_quasar_database(&conn).unwrap());
    }
}
