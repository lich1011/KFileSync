pub mod sqlite_device_repo;
pub mod sqlite_file_index_repo;
pub mod sqlite_share_repo;
pub mod sqlite_transfer_repo;

pub type Dbpool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

pub fn init_database(db_path: &str) -> Result<Dbpool, String> {
    let manager = r2d2_sqlite::SqliteConnectionManager::file(db_path);
    let pool = r2d2::Pool::builder()
        .max_size(8)
        .build(manager)
        .map_err(|e| format!("Failed to create connection pool: {}", e))?;

    let conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;",
    )
    .map_err(|e| format!("Failed to set pragmas: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS devices (
            id TEXT PRIMARY KEY,
            state_json TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("Failed to create devices table: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS audit_logs (
            id TEXT PRIMARY KEY,
            timestamp INTEGER NOT NULL,
            event_type TEXT NOT NULL,
            aggregate_id TEXT NOT NULL,
            details TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("Failed to create audit_logs table: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS transfer_jobs (
            job_id          TEXT PRIMARY KEY,
            session_id      TEXT NOT NULL,
            job_type        TEXT NOT NULL,
            peer_device_id  TEXT NOT NULL,
            share_id        TEXT,
            state_json      TEXT NOT NULL,
            status          TEXT NOT NULL,
            created_at      INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("Failed to create transfer_jobs table: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS transfer_items (
            job_id  TEXT NOT NULL,
            file_id TEXT NOT NULL,
            item_json TEXT NOT NULL,
            PRIMARY KEY (job_id, file_id),
            FOREIGN KEY (job_id) REFERENCES transfer_jobs(job_id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| format!("Failed to create transfer_items table: {}", e))?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transfer_jobs_status ON transfer_jobs(status)",
        [],
    )
    .map_err(|e| format!("Failed to create transfer_jobs index: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS shares (
            share_id    TEXT PRIMARY KEY,
            share_name  TEXT NOT NULL,
            local_path  TEXT NOT NULL,
            sync_mode   TEXT NOT NULL DEFAULT 'two_way',
            status      TEXT NOT NULL DEFAULT 'active',
            created_by  TEXT NOT NULL,
            created_at  INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("Failed to create shares table: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS share_members (
            share_id      TEXT NOT NULL,
            device_id     TEXT NOT NULL,
            permission    TEXT NOT NULL DEFAULT 'read_write',
            authorized_by TEXT NOT NULL,
            authorized_at INTEGER NOT NULL,
            PRIMARY KEY (share_id, device_id),
            FOREIGN KEY (share_id) REFERENCES shares(share_id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| format!("Failed to create share_members table: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS file_entries (
            share_id      TEXT NOT NULL,
            path          TEXT NOT NULL,
            entry_type    TEXT NOT NULL DEFAULT 'file',
            size          INTEGER NOT NULL DEFAULT 0,
            modified_at   INTEGER,
            modified_by   TEXT,
            version_vector TEXT NOT NULL DEFAULT '{}',
            sha256        TEXT,
            blocks        TEXT,
            deleted       INTEGER NOT NULL DEFAULT 0,
            deleted_at    INTEGER,
            updated_at    INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            PRIMARY KEY (share_id, path)
        )",
        [],
    )
    .map_err(|e| format!("Failed to create file_entries table: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_conflicts (
            conflict_id   TEXT PRIMARY KEY,
            share_id      TEXT NOT NULL,
            file_path     TEXT NOT NULL,
            local_entry   TEXT NOT NULL,
            remote_entry  TEXT NOT NULL,
            resolution    TEXT DEFAULT 'pending',
            created_at    INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            resolved_at   INTEGER
        )",
        [],
    )
    .map_err(|e| format!("Failed to create sync_conflicts table: {}", e))?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_file_entries_tombstone \
         ON file_entries(deleted, deleted_at) WHERE deleted = 1",
        [],
    )
    .map_err(|e| format!("Failed to create tombstone index: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS pairing_requests (
            id          TEXT PRIMARY KEY,
            device_id   TEXT NOT NULL,
            alias       TEXT,
            pin_code    TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'pending',
            created_at  INTEGER NOT NULL,
            expires_at  INTEGER NOT NULL,
            attempts    INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )
    .map_err(|e| format!("Failed to create pairing_requests table: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS config (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("Failed to create config table: {}", e))?;

    Ok(pool)
}
