//! Cron-based scheduled task execution (SSH commands on saved hosts).
//! Runs in a background task; checks every 60 seconds for due schedules. Split into the
//! task store (`store.rs`), one run of a task (`runner.rs`), and the cron tick here.

mod runner;
mod store;

pub use runner::*;
pub use store::*;

use chrono::{DateTime, Utc};
use cron::Schedule;
use log::{error, info};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::Semaphore;
use tokio::time::interval;

use crate::db;

const CHECK_INTERVAL_SECS: u64 = 60;
const SSH_TIMEOUT_SECS: u64 = 120;
/// Bounds how many scheduled tasks run at once. Without this, tasks due in the
/// same tick ran strictly sequentially, so one unreachable host could delay
/// every other due task behind its full SSH_TIMEOUT_SECS timeout.
const MAX_CONCURRENT_TASKS: usize = 5;
/// Upper bound on one scheduled SFTP transfer. Transfers had no limit, and a stalled one
/// held its concurrency slot (and, before runs were detached, the whole tick) forever
/// (audit RUST-006).
const SFTP_TASK_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// Returns true if the next occurrence of the schedule after `after` is <= `now`.
///
/// **Note:** All cron expressions are evaluated in UTC. The UI should display this
/// constraint so users schedule tasks at the correct UTC time.
fn is_due(cron_expression: &str, last_run_at: Option<i64>, now: DateTime<Utc>) -> bool {
    let schedule = match Schedule::from_str(cron_expression) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let after = last_run_at
        .and_then(|t| DateTime::from_timestamp(t, 0))
        .unwrap_or_else(|| now - chrono::Duration::seconds(2 * CHECK_INTERVAL_SECS as i64));
    let next = schedule.after(&after).next();
    match next {
        Some(t) => t <= now,
        None => false,
    }
}

/// Scheduler state that outlives a single tick.
struct SchedulerState {
    /// Shared across ticks so the concurrency bound holds for detached runs.
    semaphore: Arc<Semaphore>,
    /// When each task last started, by task id; guards against re-running a task whose
    /// `last_run_at` couldn't be persisted.
    last_executed: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
}

impl SchedulerState {
    fn new() -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_TASKS)),
            last_executed: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

async fn run_due_tasks(app: &AppHandle, state: &SchedulerState) {
    let db_path = match get_db_path(app) {
        Ok(p) => p,
        Err(e) => {
            error!("scheduler: failed to get db path: {}", e);
            return;
        }
    };
    let db_path_str = match db_path.to_str() {
        Some(s) => s.to_string(),
        None => return,
    };
    let conn = match db::open_connection(&db_path_str) {
        Ok(c) => c,
        Err(e) => {
            error!("scheduler: failed to open db: {}", e);
            return;
        }
    };

    let tasks = match load_enabled_tasks(&conn) {
        Ok(t) => t,
        Err(e) => {
            error!("scheduler: failed to load tasks: {}", e);
            return;
        }
    };
    drop(conn);

    let now = Utc::now();
    let app = app.clone();
    spawn_due_runs(tasks, now, state, move |task, now| {
        let app = app.clone();
        let db_path_str = db_path_str.clone();
        async move { run_and_record_task(&app, &db_path_str, task, now, now.timestamp()).await }
    });
}

/// Starts every due task that isn't already running, without waiting for any of them:
/// a slow or stalled run used to hold up the tick, so no other task's schedule advanced
/// until it finished (audit RUST-006). Each run holds its in-flight claim (RUST-005) and a
/// concurrency permit until it ends.
fn spawn_due_runs<R, Fut>(tasks: Vec<TaskRow>, now: DateTime<Utc>, state: &SchedulerState, run: R)
where
    R: Fn(TaskRow, DateTime<Utc>) -> Fut,
    Fut: std::future::Future<Output = Option<(String, DateTime<Utc>)>> + Send + 'static,
{
    let guard_duration = chrono::Duration::seconds(CHECK_INTERVAL_SECS as i64);
    for task in tasks {
        if !is_due(&task.cron_expression, task.last_run_at, now) {
            continue;
        }
        let recently_started = state
            .last_executed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&task.id)
            .is_some_and(|last| now.signed_duration_since(*last) < guard_duration);
        if recently_started {
            continue;
        }
        // Claimed here, synchronously, so the next tick can't start it a second time.
        let Some(running) = InFlightGuard::claim(&task.id) else {
            info!("scheduler: task '{}' is still running; skipping this tick", task.name);
            continue;
        };
        let semaphore = state.semaphore.clone();
        let last_executed = state.last_executed.clone();
        let fut = run(task, now);
        tokio::spawn(async move {
            let _running = running;
            // The semaphore is never closed, so acquire_owned only errors if that changes.
            let _permit = match semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(e) => {
                    error!("scheduler: semaphore closed unexpectedly: {}", e);
                    return;
                }
            };
            if let Some((task_id, ran_at)) = fut.await {
                last_executed
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(task_id, ran_at);
            }
        });
    }
}

/// Start the scheduler background task. Call once during app setup.
/// Uses Tauri's async runtime so it runs in the same context as other app tasks.
pub fn start_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        let state = SchedulerState::new();
        ticker.tick().await; // first tick fires immediately; skip so we wait CHECK_INTERVAL_SECS first
        loop {
            ticker.tick().await;
            run_due_tasks(&app, &state).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory DB with hosts (for FK) and scheduled_tasks schema (010 includes run-result and SFTP columns).
    fn test_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE hosts (id TEXT PRIMARY KEY, address TEXT, port INTEGER, username TEXT, protocol TEXT NOT NULL DEFAULT 'ssh')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO hosts (id, address, port, username) VALUES ('host-1', '127.0.0.1', 22, 'root')", [])
            .unwrap();
        conn.execute("INSERT INTO hosts (id, address, port, username) VALUES ('host-2', '127.0.0.1', 22, 'root')", [])
            .unwrap();
        conn.execute_batch(include_str!("../../migrations/010_scheduled_tasks.sql"))
            .unwrap();
        conn
    }

    /// PR #68 review: a host changed to an inventory-only protocol after its task was saved
    /// doesn't run the task.
    #[test]
    fn tasks_only_run_against_ssh_hosts() {
        let conn = test_conn();
        assert_eq!(get_host_credentials(&conn, "host-1").unwrap().unwrap().0, "127.0.0.1");
        assert!(get_host_credentials(&conn, "missing").unwrap().is_none());
        conn.execute("UPDATE hosts SET protocol = 'database' WHERE id = 'host-1'", []).unwrap();
        assert!(get_host_credentials(&conn, "host-1").unwrap_err().contains("not an SSH host"));
        assert!(host_protocol_runs_tasks(" SSH "));
        assert!(!host_protocol_runs_tasks("rdp"));
    }

    fn every_second_task(id: &str) -> TaskRow {
        TaskRow {
            id: id.to_string(),
            name: id.to_string(),
            cron_expression: "* * * * * *".to_string(),
            host_id: "host-1".to_string(),
            command: "true".to_string(),
            credential_id: None,
            enabled: 1,
            last_run_at: None,
            created_at: 0,
            updated_at: 0,
            task_type: "ssh".to_string(),
            local_path: None,
            remote_path: None,
        }
    }

    /// RUST-005 / RUST-006: the tick doesn't wait for runs, a stalled run doesn't hold up
    /// other tasks, and a task that is still running isn't started again.
    #[tokio::test]
    async fn due_runs_are_detached_and_never_overlap() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let state = SchedulerState::new();
        let starts: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let release_slow = Arc::new(tokio::sync::Notify::new());
        let fast_done = Arc::new(AtomicUsize::new(0));
        let runner = {
            let starts = starts.clone();
            let release_slow = release_slow.clone();
            let fast_done = fast_done.clone();
            move |task: TaskRow, now: DateTime<Utc>| {
                starts.lock().unwrap().push(task.id.clone());
                let release_slow = release_slow.clone();
                let fast_done = fast_done.clone();
                async move {
                    if task.id == "sched-test-slow" {
                        release_slow.notified().await;
                    } else {
                        fast_done.fetch_add(1, Ordering::SeqCst);
                    }
                    Some((task.id, now))
                }
            }
        };
        let tasks = || vec![every_second_task("sched-test-slow"), every_second_task("sched-test-fast")];

        let t0 = Utc::now();
        spawn_due_runs(tasks(), t0, &state, runner.clone());
        tokio::time::timeout(Duration::from_secs(5), async {
            while fast_done.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the fast task must finish while the slow one is still running");

        // Next tick: the fast task runs again, the stalled one is skipped.
        let t1 = t0 + chrono::Duration::seconds(CHECK_INTERVAL_SECS as i64 + 1);
        spawn_due_runs(tasks(), t1, &state, runner.clone());
        let slow_starts = starts.lock().unwrap().iter().filter(|id| *id == "sched-test-slow").count();
        assert_eq!(slow_starts, 1, "a running task must not start again");
        assert!(InFlightGuard::claim("sched-test-slow").is_none(), "Run now is refused too");

        // Once it finishes, it can run again.
        release_slow.notify_one();
        tokio::time::timeout(Duration::from_secs(5), async {
            while InFlightGuard::claim("sched-test-slow").is_none() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the claim is released when the run ends");
    }

    #[test]
    fn in_flight_claims_are_exclusive_until_dropped() {
        let first = InFlightGuard::claim("sched-test-claim").unwrap();
        assert!(InFlightGuard::claim("sched-test-claim").is_none());
        assert!(InFlightGuard::claim("sched-test-other").is_some());
        drop(first);
        assert!(InFlightGuard::claim("sched-test-claim").is_some());
    }

    /// FE-005: the grammar the Scheduled Tasks help text promises. The frontend only checks
    /// the field count and shows this parser's error, so this is the contract.
    #[test]
    fn cron_grammar_contract() {
        for ok in ["0 0 9 * * *", "0 */5 * * * *", "0 0 9 * * Mon-Fri", "0 30 2 1 * *", "0 0 9 * * * 2030"] {
            assert!(Schedule::from_str(ok).is_ok(), "should accept {}", ok);
        }
        for bad in ["*/5 * * * *", "0 60 * * * *", "0 0 24 * * *", "not cron"] {
            assert!(Schedule::from_str(bad).is_err(), "should reject {}", bad);
        }
    }

    #[test]
    fn test_is_due() {
        // cron crate 0.12 uses 6-field: sec min hour day month dow. "0 0 9 * * *" = daily 9:00.
        let now = Utc::now();
        let last_run = (now - chrono::Duration::hours(2)).timestamp();
        let after = DateTime::from_timestamp(last_run, 0).unwrap();
        let s = Schedule::from_str("0 0 9 * * *").unwrap();
        let next = s.after(&after).next();
        assert!(next.is_some());
    }

    #[test]
    fn test_scheduler_crud_and_run_result() {
        let conn = test_conn();

        // list empty
        let list = list_scheduled_tasks(&conn).unwrap();
        assert!(list.is_empty());

        // add task (6-field cron: sec min hour day month dow)
        let id = add_scheduled_task(
            &conn,
            "Daily backup",
            "0 0 9 * * *",
            "host-1",
            "/opt/backup.sh",
            None,
            true,
            "ssh",
            None,
            None,
        )
        .unwrap();
        assert!(!id.is_empty());

        // list returns one
        let list = list_scheduled_tasks(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Daily backup");
        assert_eq!(list[0].cron_expression, "0 0 9 * * *");
        assert_eq!(list[0].host_id, "host-1");
        assert_eq!(list[0].command, "/opt/backup.sh");
        assert!(list[0].enabled);
        assert!(list[0].last_run_at.is_none());
        assert!(list[0].last_run_status.is_none());

        // get by id
        let task = get_scheduled_task(&conn, &id).unwrap().unwrap();
        assert_eq!(task.id, id);
        assert_eq!(task.name, "Daily backup");

        // update
        update_scheduled_task(
            &conn,
            &id,
            "Daily backup v2",
            "0 0 10 * * *",
            "host-1",
            "/opt/backup.sh --full",
            None,
            true,
            "ssh",
            None,
            None,
        )
        .unwrap();
        let task = get_scheduled_task(&conn, &id).unwrap().unwrap();
        assert_eq!(task.name, "Daily backup v2");
        assert_eq!(task.cron_expression, "0 0 10 * * *");
        assert_eq!(task.command, "/opt/backup.sh --full");

        // set_run_result success
        let now_ts = Utc::now().timestamp();
        set_run_result(
            &conn,
            &id,
            now_ts,
            "success",
            None,
            Some("Backup completed."),
        )
        .unwrap();
        let task = list_scheduled_tasks(&conn)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(task.last_run_at, Some(now_ts));
        assert_eq!(task.last_run_status.as_deref(), Some("success"));
        assert!(task.last_run_error.is_none());
        assert_eq!(task.last_run_output.as_deref(), Some("Backup completed."));

        // set_run_result failure
        set_run_result(
            &conn,
            &id,
            now_ts + 1,
            "failure",
            Some("Connection refused"),
            None,
        )
        .unwrap();
        let task = get_scheduled_task(&conn, &id).unwrap().unwrap();
        assert_eq!(task.last_run_status.as_deref(), Some("failure"));
        assert_eq!(task.last_run_error.as_deref(), Some("Connection refused"));

        // remove
        remove_scheduled_task(&conn, &id).unwrap();
        assert!(get_scheduled_task(&conn, &id).unwrap().is_none());
        assert!(list_scheduled_tasks(&conn).unwrap().is_empty());
    }

    #[test]
    fn test_load_enabled_tasks_only_returns_enabled() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO hosts (id, address, port, username) VALUES ('h1', '127.0.0.1', 22, 'u')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO hosts (id, address, port, username) VALUES ('h2', '127.0.0.1', 22, 'u')",
            [],
        )
        .unwrap();
        let id1 = add_scheduled_task(
            &conn,
            "Task1",
            "0 0 * * * *",
            "h1",
            "cmd1",
            None,
            true,
            "ssh",
            None,
            None,
        )
        .unwrap();
        let id2 = add_scheduled_task(
            &conn,
            "Task2",
            "0 0 * * * *",
            "h2",
            "cmd2",
            None,
            false,
            "ssh",
            None,
            None,
        )
        .unwrap();

        let enabled = load_enabled_tasks(&conn).unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, id1);
        assert_eq!(enabled[0].name, "Task1");

        update_scheduled_task(
            &conn,
            &id2,
            "Task2",
            "0 0 * * * *",
            "h2",
            "cmd2",
            None,
            true,
            "ssh",
            None,
            None,
        )
        .unwrap();
        let enabled = load_enabled_tasks(&conn).unwrap();
        assert_eq!(enabled.len(), 2);
    }

    #[test]
    fn test_load_task_by_id() {
        let conn = test_conn();
        assert!(load_task_by_id(&conn, "nonexistent").unwrap().is_none());

        conn.execute(
            "INSERT INTO hosts (id, address, port, username) VALUES ('host', '127.0.0.1', 22, 'u')",
            [],
        )
        .unwrap();
        let id = add_scheduled_task(
            &conn,
            "One",
            "0 0 0 * * *",
            "host",
            "echo ok",
            None,
            true,
            "ssh",
            None,
            None,
        )
        .unwrap();
        let row = load_task_by_id(&conn, &id).unwrap().unwrap();
        assert_eq!(row.id, id);
        assert_eq!(row.name, "One");
        assert_eq!(row.command, "echo ok");
    }

    #[test]
    fn test_set_run_result_truncates_long_output() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO hosts (id, address, port, username) VALUES ('h', '127.0.0.1', 22, 'u')",
            [],
        )
        .unwrap();
        let id = add_scheduled_task(
            &conn,
            "Big",
            "0 0 0 * * *",
            "h",
            "cmd",
            None,
            true,
            "ssh",
            None,
            None,
        )
        .unwrap();
        let now = Utc::now().timestamp();
        let long = "x".repeat(5000);
        set_run_result(&conn, &id, now, "success", None, Some(&long)).unwrap();
        let task = get_scheduled_task(&conn, &id).unwrap().unwrap();
        assert!(task.last_run_output.as_ref().map(|s| s.len()).unwrap_or(0) <= MAX_OUTPUT_LEN + 3);
    }

    #[test]
    fn test_set_run_result_truncates_multibyte_output_without_panicking() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO hosts (id, address, port, username) VALUES ('h', '127.0.0.1', 22, 'u')",
            [],
        )
        .unwrap();
        let id = add_scheduled_task(
            &conn,
            "Unicode",
            "0 0 0 * * *",
            "h",
            "cmd",
            None,
            true,
            "ssh",
            None,
            None,
        )
        .unwrap();

        // 'é' is two bytes and straddles the MAX_OUTPUT_LEN cut point, so a naive
        // byte slice at that index panics.
        let output = format!("{}é{}", "x".repeat(MAX_OUTPUT_LEN - 1), "y".repeat(100));
        assert!(!output.is_char_boundary(MAX_OUTPUT_LEN));

        let now = Utc::now().timestamp();
        set_run_result(&conn, &id, now, "success", None, Some(&output)).unwrap();

        let task = get_scheduled_task(&conn, &id).unwrap().unwrap();
        let stored = task.last_run_output.unwrap();
        assert!(stored.ends_with("..."));
        assert!(stored.len() <= MAX_OUTPUT_LEN + 3);
    }
}
