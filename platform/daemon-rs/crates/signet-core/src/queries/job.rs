//! Memory job queue operations.
//!
//! Durable queue with lease-based processing, automatic retry, and dead-letter.

use rusqlite::{Connection, params};

use crate::error::CoreError;
use crate::types::{JobStatus, MemoryJob};

// ---------------------------------------------------------------------------
// Row parser
// ---------------------------------------------------------------------------

fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<MemoryJob> {
    let status_str: String = row.get("status")?;
    Ok(MemoryJob {
        id: row.get("id")?,
        memory_id: row.get("memory_id")?,
        job_type: row.get("job_type")?,
        status: JobStatus::from_str_lossy(&status_str),
        payload: row.get("payload")?,
        result: row.get("result")?,
        attempts: row.get::<_, i64>("attempts").unwrap_or(0),
        max_attempts: row.get::<_, i64>("max_attempts").unwrap_or(3),
        leased_at: row.get("leased_at")?,
        completed_at: row.get("completed_at")?,
        failed_at: row.get("failed_at")?,
        error: row.get("error")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        document_id: row.get("document_id")?,
    })
}

// ---------------------------------------------------------------------------
// Queue operations
// ---------------------------------------------------------------------------

pub struct EnqueueJob<'a> {
    pub id: &'a str,
    pub memory_id: Option<&'a str>,
    pub job_type: &'a str,
    pub payload: Option<&'a str>,
    pub max_attempts: i64,
    pub now: &'a str,
    pub document_id: Option<&'a str>,
}

/// Enqueue a new job. Returns the job ID.
pub fn enqueue(conn: &Connection, j: &EnqueueJob) -> Result<String, CoreError> {
    conn.execute(
        "INSERT INTO memory_jobs
         (id, memory_id, job_type, status, payload, max_attempts, document_id, created_at, updated_at)
         VALUES (?1,?2,?3,'pending',?4,?5,?6,?7,?7)",
        params![j.id, j.memory_id, j.job_type, j.payload, j.max_attempts, j.document_id, j.now],
    )?;
    Ok(j.id.to_string())
}

/// Atomically lease the next pending job of a given type.
/// Returns None if the queue is empty.
pub fn lease(conn: &Connection, job_type: &str, now: &str) -> Result<Option<MemoryJob>, CoreError> {
    let maybe_id: Option<String> = conn
        .query_row(
            "SELECT id FROM memory_jobs WHERE job_type = ?1 AND status = 'pending'
             ORDER BY created_at ASC LIMIT 1",
            params![job_type],
            |r| r.get(0),
        )
        .ok();

    let id = match maybe_id {
        Some(id) => id,
        None => return Ok(None),
    };

    conn.execute(
        "UPDATE memory_jobs SET status = 'leased', leased_at = ?1,
         attempts = COALESCE(attempts, 0) + 1, updated_at = ?1
         WHERE id = ?2",
        params![now, id],
    )?;

    // Re-fetch the updated row
    let mut stmt = conn.prepare_cached("SELECT * FROM memory_jobs WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], row_to_job)?;
    match rows.next() {
        Some(Ok(job)) => Ok(Some(job)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// Mark a job as completed.
pub fn complete(
    conn: &Connection,
    id: &str,
    result: Option<&str>,
    now: &str,
) -> Result<(), CoreError> {
    conn.execute(
        "UPDATE memory_jobs SET status = 'completed', completed_at = ?1, result = ?2, updated_at = ?1
         WHERE id = ?3",
        params![now, result, id],
    )?;
    Ok(())
}

/// Mark a job as failed. Transitions to 'dead' if max attempts exceeded.
pub fn fail(conn: &Connection, id: &str, error: &str, now: &str) -> Result<(), CoreError> {
    let (attempts, max): (i64, i64) = conn.query_row(
        "SELECT attempts, max_attempts FROM memory_jobs WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let status = if attempts >= max { "dead" } else { "failed" };

    conn.execute(
        "UPDATE memory_jobs SET status = ?1, failed_at = ?2, error = ?3, updated_at = ?2
         WHERE id = ?4",
        params![status, now, error, id],
    )?;
    Ok(())
}

/// Reset a specific job back to pending.
pub fn retry(conn: &Connection, id: &str, now: &str) -> Result<(), CoreError> {
    conn.execute(
        "UPDATE memory_jobs SET status = 'pending', error = NULL, failed_at = NULL,
         updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Requeue all dead jobs back to pending. Returns count.
pub fn requeue_dead(conn: &Connection, now: &str) -> Result<usize, CoreError> {
    let count = conn.execute(
        "UPDATE memory_jobs SET status = 'pending', attempts = 0,
         error = NULL, failed_at = NULL, updated_at = ?1
         WHERE status = 'dead'",
        params![now],
    )?;
    Ok(count)
}

/// Get job counts by status.
pub fn counts(conn: &Connection) -> Result<serde_json::Value, CoreError> {
    let mut stmt =
        conn.prepare_cached("SELECT status, count(*) FROM memory_jobs GROUP BY status")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;

    let mut map = serde_json::Map::new();
    for row in rows.flatten() {
        map.insert(row.0, serde_json::Value::from(row.1));
    }
    Ok(serde_json::Value::Object(map))
}

/// Get a job by ID.
pub fn get(conn: &Connection, id: &str) -> Result<Option<MemoryJob>, CoreError> {
    let mut stmt = conn.prepare_cached("SELECT * FROM memory_jobs WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], row_to_job)?;
    match rows.next() {
        Some(Ok(j)) => Ok(Some(j)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// List jobs filtered by type and/or status.
pub fn list_jobs(
    conn: &Connection,
    job_type: Option<&str>,
    status: Option<&str>,
    limit: usize,
) -> Result<Vec<MemoryJob>, CoreError> {
    let sql = match (job_type, status) {
        (Some(_), Some(_)) => {
            "SELECT * FROM memory_jobs WHERE job_type = ?1 AND status = ?2
             ORDER BY created_at DESC LIMIT ?3"
        }
        (Some(_), None) => {
            "SELECT * FROM memory_jobs WHERE job_type = ?1
             ORDER BY created_at DESC LIMIT ?2"
        }
        (None, Some(_)) => {
            "SELECT * FROM memory_jobs WHERE status = ?1
             ORDER BY created_at DESC LIMIT ?2"
        }
        (None, None) => "SELECT * FROM memory_jobs ORDER BY created_at DESC LIMIT ?1",
    };

    let mut stmt = conn.prepare_cached(sql)?;
    let rows = match (job_type, status) {
        (Some(jt), Some(st)) => stmt.query_map(params![jt, st, limit], row_to_job)?,
        (Some(jt), None) => stmt.query_map(params![jt, limit], row_to_job)?,
        (None, Some(st)) => stmt.query_map(params![st, limit], row_to_job)?,
        (None, None) => stmt.query_map(params![limit], row_to_job)?,
    };

    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> rusqlite::Connection {
        crate::db::register_vec_extension();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::configure_pragmas_pub(&conn).unwrap();
        crate::migrations::run(&conn).unwrap();
        crate::db::ensure_fts_pub(&conn).unwrap();
        crate::db::ensure_vec_table_pub(&conn).unwrap();
        conn
    }

    #[test]
    fn enqueue_lease_complete() {
        let conn = setup();

        // First insert a memory to satisfy FK
        conn.execute(
            "INSERT INTO memories (id, content, created_at, updated_at, updated_by, type)
             VALUES ('m1', 'test', datetime('now'), datetime('now'), 'test', 'fact')",
            [],
        )
        .unwrap();

        let job = EnqueueJob {
            id: "j1",
            memory_id: Some("m1"),
            job_type: "extraction",
            payload: Some("{\"test\":true}"),
            max_attempts: 3,
            now: "2024-01-01T00:00:00Z",
            document_id: None,
        };
        enqueue(&conn, &job).unwrap();

        // Lease
        let leased = lease(&conn, "extraction", "2024-01-01T00:01:00Z")
            .unwrap()
            .unwrap();
        assert_eq!(leased.id, "j1");
        assert_eq!(leased.status, JobStatus::Leased);
        assert_eq!(leased.attempts, 1);

        // No more to lease
        assert!(
            lease(&conn, "extraction", "2024-01-01T00:02:00Z")
                .unwrap()
                .is_none()
        );

        // Complete
        complete(&conn, "j1", Some("done"), "2024-01-01T00:03:00Z").unwrap();
        let j = get(&conn, "j1").unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Completed);
    }

    #[test]
    fn fail_and_dead_letter() {
        let conn = setup();

        conn.execute(
            "INSERT INTO memories (id, content, created_at, updated_at, updated_by, type)
             VALUES ('m2', 'test', datetime('now'), datetime('now'), 'test', 'fact')",
            [],
        )
        .unwrap();

        let job = EnqueueJob {
            id: "j2",
            memory_id: Some("m2"),
            job_type: "extraction",
            payload: None,
            max_attempts: 2,
            now: "2024-01-01T00:00:00Z",
            document_id: None,
        };
        enqueue(&conn, &job).unwrap();

        // Lease + fail
        lease(&conn, "extraction", "2024-01-01T00:01:00Z").unwrap();
        fail(&conn, "j2", "error 1", "2024-01-01T00:02:00Z").unwrap();

        let j = get(&conn, "j2").unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Failed);

        // Retry puts it back to pending
        retry(&conn, "j2", "2024-01-01T00:03:00Z").unwrap();
        let j = get(&conn, "j2").unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Pending);

        // Lease again (bumps attempts to 2) + fail → dead
        lease(&conn, "extraction", "2024-01-01T00:04:00Z").unwrap();
        fail(&conn, "j2", "error 2", "2024-01-01T00:05:00Z").unwrap();

        let j = get(&conn, "j2").unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Dead);

        // Requeue dead
        let n = requeue_dead(&conn, "2024-01-01T00:06:00Z").unwrap();
        assert_eq!(n, 1);
        let j = get(&conn, "j2").unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Pending);
    }
}
