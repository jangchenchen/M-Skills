use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use skillsmgr_core::{
    Artifact, Installation, Result, ScannedInstallation, SkillsMgrError, Source, Status,
};
use uuid::Uuid;

pub struct Registry {
    connection: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryArtifact {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub installed_version: Option<String>,
    pub on_disk_path: Option<PathBuf>,
    pub source_url: Option<String>,
    pub commit_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryInstallation {
    pub id: Uuid,
    pub artifact_id: Uuid,
    pub target: String,
    pub status: String,
    pub installed_version: Option<String>,
    pub on_disk_path: PathBuf,
    pub installed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryTranslation {
    pub id: String,
    pub artifact_name: String,
    pub file_path: PathBuf,
    pub field: String,
    pub source_sha256: String,
    pub locale: String,
    pub translated_text: String,
    pub translated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationInput {
    pub artifact_name: String,
    pub file_path: PathBuf,
    pub field: String,
    pub source_sha256: String,
    pub locale: String,
    pub translated_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrySkillSummary {
    pub id: String,
    pub skill_name: String,
    pub source_sha256: String,
    pub locale: String,
    pub summary_json: String,
    pub model: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSummaryInput {
    pub skill_name: String,
    pub source_sha256: String,
    pub locale: String,
    pub summary_json: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEvent {
    pub id: Uuid,
    pub event_type: String,
    pub artifact_name: Option<String>,
    pub target: Option<String>,
    pub succeeded: bool,
    pub error_message: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

pub struct RecordEventInput {
    pub event_type: String,
    pub artifact_name: Option<String>,
    pub target: Option<String>,
    pub succeeded: bool,
    pub error_message: Option<String>,
}

impl Registry {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| fs_error(parent, source))?;
        }
        let connection = Connection::open(path).map_err(registry_error)?;
        let registry = Self { connection };
        registry.migrate()?;
        Ok(registry)
    }

    pub fn in_memory() -> Result<Self> {
        let registry = Self {
            connection: Connection::open_in_memory().map_err(registry_error)?,
        };
        registry.migrate()?;
        Ok(registry)
    }

    pub fn upsert_scan_results(&mut self, scanned: &[ScannedInstallation]) -> Result<()> {
        let transaction = self.connection.transaction().map_err(registry_error)?;
        for item in scanned {
            upsert_artifact_tx(&transaction, &item.artifact, "installed")?;
            upsert_source_metadata_tx(&transaction, &item.artifact)?;
            upsert_installation_tx(&transaction, &item.installation)?;
        }
        transaction.commit().map_err(registry_error)
    }

    pub fn record_installation(
        &mut self,
        artifact: &Artifact,
        installation: &Installation,
    ) -> Result<()> {
        let transaction = self.connection.transaction().map_err(registry_error)?;
        upsert_artifact_tx(&transaction, artifact, "installed")?;
        upsert_source_metadata_tx(&transaction, artifact)?;
        upsert_installation_tx(&transaction, installation)?;
        transaction.commit().map_err(registry_error)
    }

    pub fn record_uninstall(&self, installation_id: Uuid) -> Result<()> {
        self.connection
            .execute(
                "UPDATE installations SET status = 'removed' WHERE id = ?1",
                params![installation_id.to_string()],
            )
            .map_err(registry_error)?;
        Ok(())
    }

    pub fn artifact_name_by_id(&self, artifact_id: Uuid) -> Option<String> {
        self.connection
            .query_row(
                "SELECT name FROM artifacts WHERE id = ?1",
                params![artifact_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn record_event(&mut self, input: RecordEventInput) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let occurred_at = Utc::now().to_rfc3339();
        self.connection
            .execute(
                "INSERT INTO events (id, event_type, artifact_name, target, succeeded, error_message, occurred_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    input.event_type,
                    input.artifact_name,
                    input.target,
                    input.succeeded as i32,
                    input.error_message,
                    occurred_at,
                ],
            )
            .map_err(registry_error)?;
        Ok(())
    }

    pub fn recent_events(&self, limit: usize) -> Result<Vec<RegistryEvent>> {
        let mut stmt = self
            .connection
            .prepare(
                "SELECT id, event_type, artifact_name, target, succeeded, error_message, occurred_at
                 FROM events
                 ORDER BY occurred_at DESC
                 LIMIT ?1",
            )
            .map_err(registry_error)?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let id_str: String = row.get(0)?;
                let occurred_at_str: String = row.get(6)?;
                Ok((
                    id_str,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    occurred_at_str,
                ))
            })
            .map_err(registry_error)?;

        let mut events = Vec::new();
        for row in rows {
            let (
                id_str,
                event_type,
                artifact_name,
                target,
                succeeded,
                error_message,
                occurred_at_str,
            ) = row.map_err(registry_error)?;
            let id =
                Uuid::parse_str(&id_str).map_err(|e| SkillsMgrError::Registry(e.to_string()))?;
            let occurred_at = DateTime::parse_from_rfc3339(&occurred_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            events.push(RegistryEvent {
                id,
                event_type,
                artifact_name,
                target,
                succeeded: succeeded != 0,
                error_message,
                occurred_at,
            });
        }
        Ok(events)
    }

    pub fn stale_installation_count(&self) -> Result<usize> {
        let mut stmt = self
            .connection
            .prepare(
                "SELECT on_disk_path FROM installations WHERE status = 'installed' OR status = 'enabled'",
            )
            .map_err(registry_error)?;
        let paths: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(registry_error)?
            .filter_map(|r| r.ok())
            .collect();
        let stale = paths
            .iter()
            .filter(|p| !std::path::Path::new(p).exists())
            .count();
        Ok(stale)
    }

    pub fn artifact_by_name(&self, name: &str) -> Result<Option<RegistryArtifact>> {
        self.connection
            .query_row(
                "SELECT a.id, a.name, a.kind, a.status, a.installed_version, \
                        a.on_disk_path, s.source_url, s.commit_sha \
                 FROM artifacts a \
                 LEFT JOIN source_metadata s ON s.artifact_id = a.id \
                 WHERE a.name = ?1",
                params![name],
                read_registry_artifact,
            )
            .optional()
            .map_err(registry_error)
    }

    pub fn installations(&self) -> Result<Vec<RegistryInstallation>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, artifact_id, target, status, installed_version, on_disk_path, installed_at \
                 FROM installations ORDER BY target, on_disk_path",
            )
            .map_err(registry_error)?;
        let rows = statement
            .query_map([], read_registry_installation)
            .map_err(registry_error)?;

        let mut installations = Vec::new();
        for row in rows {
            installations.push(row.map_err(registry_error)?);
        }
        Ok(installations)
    }

    pub fn translation(
        &self,
        artifact_name: &str,
        file_path: &Path,
        field: &str,
        source_sha256: &str,
        locale: &str,
    ) -> Result<Option<RegistryTranslation>> {
        self.connection
            .query_row(
                "SELECT id, artifact_name, file_path, field, source_sha256, locale, translated_text, translated_at
                 FROM translations
                 WHERE artifact_name = ?1
                   AND file_path = ?2
                   AND field = ?3
                   AND source_sha256 = ?4
                   AND locale = ?5",
                params![
                    artifact_name,
                    file_path.to_string_lossy(),
                    field,
                    source_sha256,
                    locale,
                ],
                read_registry_translation,
            )
            .optional()
            .map_err(registry_error)
    }

    pub fn clear_translations(
        &self,
        artifact_name: &str,
        file_path: &Path,
        field: &str,
        locale: &str,
    ) -> Result<usize> {
        let deleted = self
            .connection
            .execute(
                "DELETE FROM translations
                 WHERE artifact_name = ?1
                   AND file_path = ?2
                   AND field = ?3
                   AND locale = ?4",
                params![artifact_name, file_path.to_string_lossy(), field, locale,],
            )
            .map_err(registry_error)?;
        Ok(deleted)
    }

    pub fn upsert_translation(&self, input: &TranslationInput) -> Result<RegistryTranslation> {
        let translated_at = Utc::now();
        let id = translation_id(
            &input.artifact_name,
            &input.file_path,
            &input.field,
            &input.source_sha256,
            &input.locale,
        );

        let tx = self
            .connection
            .unchecked_transaction()
            .map_err(registry_error)?;
        tx.execute(
            "INSERT INTO translations (
                    id, artifact_name, file_path, field, source_sha256, locale, translated_text, translated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(artifact_name, file_path, field, source_sha256, locale) DO UPDATE SET
                    translated_text = excluded.translated_text,
                    translated_at = excluded.translated_at",
            params![
                id,
                input.artifact_name,
                input.file_path.to_string_lossy(),
                input.field,
                input.source_sha256,
                input.locale,
                input.translated_text,
                translated_at.to_rfc3339(),
            ],
        )
        .map_err(registry_error)?;
        tx.execute(
            "DELETE FROM translations
             WHERE artifact_name = ?1
               AND file_path = ?2
               AND field = ?3
               AND locale = ?4
               AND source_sha256 <> ?5",
            params![
                input.artifact_name,
                input.file_path.to_string_lossy(),
                input.field,
                input.locale,
                input.source_sha256,
            ],
        )
        .map_err(registry_error)?;
        tx.commit().map_err(registry_error)?;

        Ok(RegistryTranslation {
            id,
            artifact_name: input.artifact_name.clone(),
            file_path: input.file_path.clone(),
            field: input.field.clone(),
            source_sha256: input.source_sha256.clone(),
            locale: input.locale.clone(),
            translated_text: input.translated_text.clone(),
            translated_at,
        })
    }

    pub fn skill_summary(
        &self,
        skill_name: &str,
        source_sha256: &str,
        locale: &str,
    ) -> Result<Option<RegistrySkillSummary>> {
        self.connection
            .query_row(
                "SELECT id, skill_name, source_sha256, locale, summary_json, model, generated_at
                 FROM skill_summaries
                 WHERE skill_name = ?1 AND source_sha256 = ?2 AND locale = ?3",
                params![skill_name, source_sha256, locale],
                read_registry_skill_summary,
            )
            .optional()
            .map_err(registry_error)
    }

    pub fn upsert_skill_summary(&self, input: &SkillSummaryInput) -> Result<RegistrySkillSummary> {
        let generated_at = Utc::now();
        let id = skill_summary_id(&input.skill_name, &input.source_sha256, &input.locale);

        let tx = self
            .connection
            .unchecked_transaction()
            .map_err(registry_error)?;
        tx.execute(
            "INSERT INTO skill_summaries (
                    id, skill_name, source_sha256, locale, summary_json, model, generated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(skill_name, source_sha256, locale) DO UPDATE SET
                    summary_json = excluded.summary_json,
                    model = excluded.model,
                    generated_at = excluded.generated_at",
            params![
                id,
                input.skill_name,
                input.source_sha256,
                input.locale,
                input.summary_json,
                input.model,
                generated_at.to_rfc3339(),
            ],
        )
        .map_err(registry_error)?;
        tx.execute(
            "DELETE FROM skill_summaries
             WHERE skill_name = ?1
               AND locale = ?2
               AND source_sha256 <> ?3",
            params![input.skill_name, input.locale, input.source_sha256],
        )
        .map_err(registry_error)?;
        tx.commit().map_err(registry_error)?;

        Ok(RegistrySkillSummary {
            id,
            skill_name: input.skill_name.clone(),
            source_sha256: input.source_sha256.clone(),
            locale: input.locale.clone(),
            summary_json: input.summary_json.clone(),
            model: input.model.clone(),
            generated_at,
        })
    }

    pub fn clear_skill_summary(&self, skill_name: &str, locale: &str) -> Result<usize> {
        let deleted = self
            .connection
            .execute(
                "DELETE FROM skill_summaries WHERE skill_name = ?1 AND locale = ?2",
                params![skill_name, locale],
            )
            .map_err(registry_error)?;
        Ok(deleted)
    }

    fn migrate(&self) -> Result<()> {
        self.connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS artifacts (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    status TEXT NOT NULL,
                    installed_version TEXT,
                    on_disk_path TEXT,
                    source_json TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS installations (
                    id TEXT PRIMARY KEY,
                    artifact_id TEXT NOT NULL,
                    target TEXT NOT NULL,
                    status TEXT NOT NULL,
                    installed_version TEXT,
                    on_disk_path TEXT NOT NULL,
                    installed_at TEXT NOT NULL,
                    target_json TEXT NOT NULL,
                    FOREIGN KEY (artifact_id) REFERENCES artifacts(id)
                );

                CREATE TABLE IF NOT EXISTS source_metadata (
                    artifact_id TEXT PRIMARY KEY,
                    source_url TEXT,
                    commit_sha TEXT,
                    local_path TEXT,
                    source_json TEXT NOT NULL,
                    FOREIGN KEY (artifact_id) REFERENCES artifacts(id)
                );

                CREATE TABLE IF NOT EXISTS translations (
                    id TEXT PRIMARY KEY,
                    artifact_name TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    field TEXT NOT NULL,
                    source_sha256 TEXT NOT NULL,
                    locale TEXT NOT NULL,
                    translated_text TEXT NOT NULL,
                    translated_at TEXT NOT NULL
                );

                CREATE UNIQUE INDEX IF NOT EXISTS translations_lookup
                    ON translations (artifact_name, file_path, field, source_sha256, locale);

                CREATE TABLE IF NOT EXISTS skill_summaries (
                    id TEXT PRIMARY KEY,
                    skill_name TEXT NOT NULL,
                    source_sha256 TEXT NOT NULL,
                    locale TEXT NOT NULL,
                    summary_json TEXT NOT NULL,
                    model TEXT NOT NULL,
                    generated_at TEXT NOT NULL
                );

                CREATE UNIQUE INDEX IF NOT EXISTS skill_summaries_lookup
                    ON skill_summaries (skill_name, source_sha256, locale);

                CREATE TABLE IF NOT EXISTS events (
                    id TEXT PRIMARY KEY,
                    event_type TEXT NOT NULL,
                    artifact_name TEXT,
                    target TEXT,
                    succeeded INTEGER NOT NULL,
                    error_message TEXT,
                    occurred_at TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS events_occurred_at
                    ON events (occurred_at DESC);
                ",
            )
            .map_err(registry_error)
    }
}

fn upsert_artifact_tx(connection: &Connection, artifact: &Artifact, status: &str) -> Result<()> {
    let source_json = serde_json::to_string(&artifact.source).map_err(json_error)?;
    connection
        .execute(
            "INSERT INTO artifacts (
                id, name, description, kind, status, installed_version, on_disk_path, source_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                kind = excluded.kind,
                status = excluded.status,
                installed_version = excluded.installed_version,
                source_json = excluded.source_json",
            params![
                artifact.id.to_string(),
                artifact.name,
                artifact.description,
                format!("{:?}", artifact.kind),
                status,
                artifact.version,
                source_json,
            ],
        )
        .map_err(registry_error)?;
    Ok(())
}

fn upsert_installation_tx(connection: &Connection, installation: &Installation) -> Result<()> {
    let target_json = serde_json::to_string(&installation.target).map_err(json_error)?;
    connection
        .execute(
            "INSERT INTO installations (
                id, artifact_id, target, status, installed_version, on_disk_path, installed_at, target_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                artifact_id = excluded.artifact_id,
                target = excluded.target,
                status = excluded.status,
                installed_version = excluded.installed_version,
                on_disk_path = excluded.on_disk_path,
                installed_at = excluded.installed_at,
                target_json = excluded.target_json",
            params![
                installation.id.to_string(),
                installation.artifact_id.to_string(),
                installation.target.tool_id(),
                status_label(&installation.status),
                installation.installed_version,
                installation.on_disk_path.to_string_lossy(),
                installation.installed_at.to_rfc3339(),
                target_json,
            ],
        )
        .map_err(registry_error)?;
    connection
        .execute(
            "UPDATE artifacts
             SET status = ?1,
                 installed_version = ?2,
                 on_disk_path = ?3
             WHERE id = ?4",
            params![
                status_label(&installation.status),
                installation.installed_version,
                installation.on_disk_path.to_string_lossy(),
                installation.artifact_id.to_string(),
            ],
        )
        .map_err(registry_error)?;
    Ok(())
}

fn upsert_source_metadata_tx(connection: &Connection, artifact: &Artifact) -> Result<()> {
    let (source_url, commit_sha, local_path) = match &artifact.source {
        Source::GitHub { url, rev } => (Some(url.clone()), Some(rev.clone()), None),
        Source::Url { url } => (Some(url.clone()), None, None),
        Source::Local { path } => (None, None, Some(path.to_string_lossy().to_string())),
        Source::Bundled | Source::Unknown => (None, None, None),
    };
    let source_json = serde_json::to_string(&artifact.source).map_err(json_error)?;
    connection
        .execute(
            "INSERT INTO source_metadata (
                artifact_id, source_url, commit_sha, local_path, source_json
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(artifact_id) DO UPDATE SET
                source_url = excluded.source_url,
                commit_sha = excluded.commit_sha,
                local_path = excluded.local_path,
                source_json = excluded.source_json",
            params![
                artifact.id.to_string(),
                source_url,
                commit_sha,
                local_path,
                source_json,
            ],
        )
        .map_err(registry_error)?;
    Ok(())
}

fn read_registry_artifact(row: &rusqlite::Row<'_>) -> rusqlite::Result<RegistryArtifact> {
    Ok(RegistryArtifact {
        id: parse_uuid_row(row, 0)?,
        name: row.get(1)?,
        kind: row.get(2)?,
        status: row.get(3)?,
        installed_version: row.get(4)?,
        on_disk_path: row.get::<_, Option<String>>(5)?.map(PathBuf::from),
        source_url: row.get(6)?,
        commit_sha: row.get(7)?,
    })
}

fn read_registry_installation(row: &rusqlite::Row<'_>) -> rusqlite::Result<RegistryInstallation> {
    let installed_at: String = row.get(6)?;
    let installed_at = DateTime::parse_from_rfc3339(&installed_at)
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?
        .with_timezone(&Utc);

    Ok(RegistryInstallation {
        id: parse_uuid_row(row, 0)?,
        artifact_id: parse_uuid_row(row, 1)?,
        target: row.get(2)?,
        status: row.get(3)?,
        installed_version: row.get(4)?,
        on_disk_path: PathBuf::from(row.get::<_, String>(5)?),
        installed_at,
    })
}

fn read_registry_translation(row: &rusqlite::Row<'_>) -> rusqlite::Result<RegistryTranslation> {
    let translated_at: String = row.get(7)?;
    let translated_at = DateTime::parse_from_rfc3339(&translated_at)
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?
        .with_timezone(&Utc);

    Ok(RegistryTranslation {
        id: row.get(0)?,
        artifact_name: row.get(1)?,
        file_path: PathBuf::from(row.get::<_, String>(2)?),
        field: row.get(3)?,
        source_sha256: row.get(4)?,
        locale: row.get(5)?,
        translated_text: row.get(6)?,
        translated_at,
    })
}

fn read_registry_skill_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<RegistrySkillSummary> {
    let generated_at: String = row.get(6)?;
    let generated_at = DateTime::parse_from_rfc3339(&generated_at)
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?
        .with_timezone(&Utc);
    Ok(RegistrySkillSummary {
        id: row.get(0)?,
        skill_name: row.get(1)?,
        source_sha256: row.get(2)?,
        locale: row.get(3)?,
        summary_json: row.get(4)?,
        model: row.get(5)?,
        generated_at,
    })
}

fn parse_uuid_row(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Uuid> {
    let value: String = row.get(index)?;
    Uuid::parse_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn translation_id(
    artifact_name: &str,
    file_path: &Path,
    field: &str,
    source_sha256: &str,
    locale: &str,
) -> String {
    format!(
        "{artifact_name}:{}:{field}:{source_sha256}:{locale}",
        file_path.to_string_lossy()
    )
}

fn skill_summary_id(skill_name: &str, source_sha256: &str, locale: &str) -> String {
    format!("{skill_name}:{source_sha256}:{locale}")
}

fn status_label(status: &Status) -> String {
    match status {
        Status::Enabled => "enabled".to_string(),
        Status::Disabled => "disabled".to_string(),
        Status::Broken { reason } => format!("broken:{reason}"),
    }
}

fn fs_error(path: impl AsRef<Path>, source: std::io::Error) -> SkillsMgrError {
    SkillsMgrError::Fs {
        path: path.as_ref().to_path_buf(),
        source,
    }
}

fn registry_error(error: rusqlite::Error) -> SkillsMgrError {
    SkillsMgrError::Registry(error.to_string())
}

fn json_error(error: serde_json::Error) -> SkillsMgrError {
    SkillsMgrError::Registry(error.to_string())
}

#[cfg(test)]
mod tests {
    use skillsmgr_core::{ArtifactKind, Scope, Source, Target};

    use super::*;

    #[test]
    fn rebuilds_registry_from_scan_results() {
        let mut registry = Registry::in_memory().unwrap();
        let artifact = Artifact::new(
            "demo",
            "Demo",
            Some("1.0.0".to_string()),
            ArtifactKind::Skill,
            Source::GitHub {
                url: "https://github.com/example/demo".to_string(),
                rev: "abc123".to_string(),
            },
        );
        let installation = Installation::enabled(
            &artifact,
            Target::Codex {
                scope: Scope::Global,
            },
            "/tmp/demo",
        );
        let scanned = ScannedInstallation {
            artifact: artifact.clone(),
            installation: installation.clone(),
            provenance: skillsmgr_core::SourceProvenance::Owned,
        };

        registry.upsert_scan_results(&[scanned]).unwrap();

        let stored = registry.artifact_by_name("demo").unwrap().unwrap();
        assert_eq!(stored.name, "demo");
        assert_eq!(stored.kind, "Skill");
        assert_eq!(stored.status, "enabled");
        assert_eq!(stored.installed_version.as_deref(), Some("1.0.0"));
        assert_eq!(
            stored.source_url.as_deref(),
            Some("https://github.com/example/demo")
        );
        assert_eq!(stored.commit_sha.as_deref(), Some("abc123"));

        let installations = registry.installations().unwrap();
        assert_eq!(installations.len(), 1);
        assert_eq!(installations[0].target, "codex");
    }

    #[test]
    fn install_updates_registry_after_filesystem_success() {
        let mut registry = Registry::in_memory().unwrap();
        let artifact = Artifact::new(
            "demo",
            "Demo",
            None,
            ArtifactKind::Skill,
            Source::Local {
                path: "/tmp/source".into(),
            },
        );
        let installation = Installation::enabled(
            &artifact,
            Target::ClaudeCode {
                scope: Scope::Global,
            },
            "/tmp/installed/demo",
        );

        registry
            .record_installation(&artifact, &installation)
            .unwrap();

        let stored = registry.artifact_by_name("demo").unwrap().unwrap();
        assert_eq!(
            stored.on_disk_path.as_deref(),
            Some(Path::new("/tmp/installed/demo"))
        );
        assert_eq!(registry.installations().unwrap()[0].status, "enabled");
    }

    #[test]
    fn uninstall_marks_installation_removed() {
        let mut registry = Registry::in_memory().unwrap();
        let artifact = Artifact::new("demo", "Demo", None, ArtifactKind::Skill, Source::Unknown);
        let installation = Installation::enabled(
            &artifact,
            Target::ClaudeCode {
                scope: Scope::Global,
            },
            "/tmp/installed/demo",
        );
        registry
            .record_installation(&artifact, &installation)
            .unwrap();

        registry.record_uninstall(installation.id).unwrap();

        assert_eq!(registry.installations().unwrap()[0].status, "removed");
    }

    #[test]
    fn upserts_and_reads_translations() {
        let registry = Registry::in_memory().unwrap();
        let input = TranslationInput {
            artifact_name: "polish-code".to_string(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".to_string(),
            source_sha256: "abc123".to_string(),
            locale: "zh".to_string(),
            translated_text: "中文".to_string(),
        };

        let stored = registry.upsert_translation(&input).unwrap();
        let fetched = registry
            .translation("polish-code", Path::new("SKILL.md"), "body", "abc123", "zh")
            .unwrap()
            .unwrap();

        assert_eq!(stored.id, fetched.id);
        assert_eq!(fetched.translated_text, "中文");
        assert_eq!(fetched.locale, "zh");
    }

    #[test]
    fn clear_translations_removes_only_matching_combination() {
        let registry = Registry::in_memory().unwrap();

        let base = TranslationInput {
            artifact_name: "skill-a".to_string(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".to_string(),
            source_sha256: "sha1".to_string(),
            locale: "zh".to_string(),
            translated_text: "zh-body".to_string(),
        };
        registry.upsert_translation(&base).unwrap();
        registry
            .upsert_translation(&TranslationInput {
                artifact_name: "skill-b".to_string(),
                source_sha256: "sha-b".to_string(),
                translated_text: "other skill".to_string(),
                ..base.clone()
            })
            .unwrap();
        registry
            .upsert_translation(&TranslationInput {
                file_path: PathBuf::from("OTHER.md"),
                source_sha256: "sha-other-file".to_string(),
                translated_text: "other file".to_string(),
                ..base.clone()
            })
            .unwrap();
        registry
            .upsert_translation(&TranslationInput {
                field: "title".to_string(),
                source_sha256: "sha-title".to_string(),
                translated_text: "other field".to_string(),
                ..base.clone()
            })
            .unwrap();
        registry
            .upsert_translation(&TranslationInput {
                locale: "ja".to_string(),
                source_sha256: "sha-ja".to_string(),
                translated_text: "ja-body".to_string(),
                ..base.clone()
            })
            .unwrap();

        let deleted = registry
            .clear_translations("skill-a", Path::new("SKILL.md"), "body", "zh")
            .unwrap();
        assert_eq!(deleted, 1);

        assert!(registry
            .translation("skill-a", Path::new("SKILL.md"), "body", "sha1", "zh")
            .unwrap()
            .is_none());
        assert!(registry
            .translation("skill-b", Path::new("SKILL.md"), "body", "sha-b", "zh")
            .unwrap()
            .is_some());
        assert!(registry
            .translation(
                "skill-a",
                Path::new("OTHER.md"),
                "body",
                "sha-other-file",
                "zh"
            )
            .unwrap()
            .is_some());
        assert!(registry
            .translation("skill-a", Path::new("SKILL.md"), "title", "sha-title", "zh")
            .unwrap()
            .is_some());
        assert!(registry
            .translation("skill-a", Path::new("SKILL.md"), "body", "sha-ja", "ja")
            .unwrap()
            .is_some());

        // Clearing again is a no-op (0 deleted).
        let deleted_again = registry
            .clear_translations("skill-a", Path::new("SKILL.md"), "body", "zh")
            .unwrap();
        assert_eq!(deleted_again, 0);
    }

    #[test]
    fn upserts_and_reads_skill_summary() {
        let registry = Registry::in_memory().unwrap();
        let input = SkillSummaryInput {
            skill_name: "polish-code".into(),
            source_sha256: "abc".into(),
            locale: "en".into(),
            summary_json: r#"{"commands":["lint"],"capabilities":"Lints code.","useCases":["pre-commit"],"examples":["/lint"]}"#.into(),
            model: "gpt-4o-mini".into(),
        };
        let stored = registry.upsert_skill_summary(&input).unwrap();
        let fetched = registry
            .skill_summary("polish-code", "abc", "en")
            .unwrap()
            .unwrap();
        assert_eq!(stored.id, fetched.id);
        assert_eq!(fetched.summary_json, input.summary_json);
        assert_eq!(fetched.model, "gpt-4o-mini");
    }

    #[test]
    fn clear_skill_summary_removes_only_matching_skill_locale() {
        let registry = Registry::in_memory().unwrap();
        registry
            .upsert_skill_summary(&SkillSummaryInput {
                skill_name: "skill-a".into(),
                source_sha256: "sha1".into(),
                locale: "en".into(),
                summary_json: "{}".into(),
                model: "m".into(),
            })
            .unwrap();
        registry
            .upsert_skill_summary(&SkillSummaryInput {
                skill_name: "skill-a".into(),
                source_sha256: "sha2".into(),
                locale: "zh".into(),
                summary_json: "{}".into(),
                model: "m".into(),
            })
            .unwrap();
        registry
            .upsert_skill_summary(&SkillSummaryInput {
                skill_name: "skill-b".into(),
                source_sha256: "sha3".into(),
                locale: "en".into(),
                summary_json: "{}".into(),
                model: "m".into(),
            })
            .unwrap();

        let deleted = registry.clear_skill_summary("skill-a", "en").unwrap();
        assert_eq!(deleted, 1);

        assert!(registry
            .skill_summary("skill-a", "sha1", "en")
            .unwrap()
            .is_none());
        assert!(registry
            .skill_summary("skill-a", "sha2", "zh")
            .unwrap()
            .is_some());
        assert!(registry
            .skill_summary("skill-b", "sha3", "en")
            .unwrap()
            .is_some());
    }

    #[test]
    fn upsert_purges_stale_skill_summary_for_same_skill_locale() {
        let registry = Registry::in_memory().unwrap();
        registry
            .upsert_skill_summary(&SkillSummaryInput {
                skill_name: "polish-code".into(),
                source_sha256: "old".into(),
                locale: "en".into(),
                summary_json: r#"{"capabilities":"old"}"#.into(),
                model: "m".into(),
            })
            .unwrap();
        // Different locale must survive.
        registry
            .upsert_skill_summary(&SkillSummaryInput {
                skill_name: "polish-code".into(),
                source_sha256: "old-zh".into(),
                locale: "zh".into(),
                summary_json: r#"{"capabilities":"旧"}"#.into(),
                model: "m".into(),
            })
            .unwrap();

        // New source hash for same (name, locale) should evict the old row.
        registry
            .upsert_skill_summary(&SkillSummaryInput {
                skill_name: "polish-code".into(),
                source_sha256: "new".into(),
                locale: "en".into(),
                summary_json: r#"{"capabilities":"new"}"#.into(),
                model: "m".into(),
            })
            .unwrap();

        assert!(registry
            .skill_summary("polish-code", "old", "en")
            .unwrap()
            .is_none());
        let now = registry
            .skill_summary("polish-code", "new", "en")
            .unwrap()
            .unwrap();
        assert!(now.summary_json.contains("new"));
        // Different-locale entry survives.
        assert!(registry
            .skill_summary("polish-code", "old-zh", "zh")
            .unwrap()
            .is_some());
    }

    #[test]
    fn upsert_purges_stale_translations_for_same_skill_locale() {
        let registry = Registry::in_memory().unwrap();

        let v1 = TranslationInput {
            artifact_name: "polish-code".to_string(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".to_string(),
            source_sha256: "sha-v1".to_string(),
            locale: "zh".to_string(),
            translated_text: "旧译文".to_string(),
        };
        let other_locale = TranslationInput {
            locale: "ja".to_string(),
            translated_text: "古い翻訳".to_string(),
            source_sha256: "sha-v1-ja".to_string(),
            ..v1.clone()
        };
        let other_skill = TranslationInput {
            artifact_name: "different-skill".to_string(),
            translated_text: "别的 skill".to_string(),
            source_sha256: "sha-other".to_string(),
            ..v1.clone()
        };
        registry.upsert_translation(&v1).unwrap();
        registry.upsert_translation(&other_locale).unwrap();
        registry.upsert_translation(&other_skill).unwrap();

        let v2 = TranslationInput {
            source_sha256: "sha-v2".to_string(),
            translated_text: "新译文".to_string(),
            ..v1.clone()
        };
        registry.upsert_translation(&v2).unwrap();

        // Stale sha for same skill+locale is gone.
        assert!(registry
            .translation("polish-code", Path::new("SKILL.md"), "body", "sha-v1", "zh")
            .unwrap()
            .is_none());

        // New sha is present.
        let now = registry
            .translation("polish-code", Path::new("SKILL.md"), "body", "sha-v2", "zh")
            .unwrap()
            .unwrap();
        assert_eq!(now.translated_text, "新译文");

        // Different-locale entry survives.
        let ja = registry
            .translation(
                "polish-code",
                Path::new("SKILL.md"),
                "body",
                "sha-v1-ja",
                "ja",
            )
            .unwrap()
            .unwrap();
        assert_eq!(ja.translated_text, "古い翻訳");

        // Different-skill entry survives.
        let other = registry
            .translation(
                "different-skill",
                Path::new("SKILL.md"),
                "body",
                "sha-other",
                "zh",
            )
            .unwrap()
            .unwrap();
        assert_eq!(other.translated_text, "别的 skill");
    }
}
