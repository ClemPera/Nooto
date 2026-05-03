use anyhow::{Context, Result};
use mysql_async::{Conn, params, prelude::Queryable};

/// Each migration is a (version, sql) pair. Version must be monotonically increasing.
/// To add a migration: create a `VN__description.sql` file in `migrations/`, then
/// append a new entry here using `include_str!`. Never edit existing entries.
static MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/V1__init.sql")),
];

/// Creates the tracking table if absent, then runs every migration whose version
/// is not yet recorded, in order.
pub async fn run(conn: &mut Conn) -> Result<()> {
    println!("Running database migrations...");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `schema_migrations` (
            `version`    INT UNSIGNED NOT NULL,
            `applied_at` BIGINT       NOT NULL,
            PRIMARY KEY (`version`)
        )",
    )
    .await
    .context("Failed to create schema_migrations table")?;

    let applied: Vec<u32> = conn
        .query("SELECT version FROM schema_migrations ORDER BY version")
        .await
        .context("Failed to query applied migrations")?;

    let mut applied_count = 0u32;

    for (version, sql) in MIGRATIONS {
        if applied.contains(version) {
            continue;
        }

        println!("Applying migration V{version}...");

        for statement in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            conn.query_drop(statement)
                .await
                .with_context(|| format!("Migration V{version} failed on statement: {statement}"))?;
        }

        conn.exec_drop(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (:version, :applied_at)",
            params! {
                "version" => version,
                "applied_at" => chrono::Local::now().to_utc().timestamp(),
            },
        )
        .await
        .with_context(|| format!("Failed to record migration V{version}"))?;

        println!("Migration V{version} applied successfully");
        applied_count += 1;
    }

    if applied_count == 0 {
        println!(
            "Database schema up to date ({} migration{} already applied)",
            applied.len(),
            if applied.len() == 1 { "" } else { "s" }
        );
    } else {
        println!("{applied_count} migration{} applied", if applied_count == 1 { "" } else { "s" });
    }

    Ok(())
}
