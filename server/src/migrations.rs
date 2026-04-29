use anyhow::{Context, Result};
use mysql_async::{Conn, params, prelude::Queryable};

/// Each migration is a (version, sql) pair. Version must be monotonically increasing.
/// Append new entries here to add future migrations; never edit existing ones.
static MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/V1__init.sql")),
];

/// Creates the tracking table if absent, then runs every migration whose version
/// is not yet recorded, in order.
pub async fn run(conn: &mut Conn) -> Result<()> {
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

    for (version, sql) in MIGRATIONS {
        if applied.contains(version) {
            continue;
        }

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

        println!("Applied migration V{version}");
    }

    Ok(())
}
