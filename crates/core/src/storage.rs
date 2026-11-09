use crate::error::Result;
use crate::model::{Segment, TranscriptionResult};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub filename: String,
    pub engine: String,
    pub segment_count: i64,
    pub duration_ms: i64,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
}

impl HistoryStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let store = Self { path: path.into() };
        store.initialize()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn connect(&self) -> Result<Connection> {
        let connection = Connection::open(&self.path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(connection)
    }

    fn initialize(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = self.connect()?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                engine TEXT NOT NULL,
                language TEXT,
                duration_ms INTEGER NOT NULL,
                segment_count INTEGER NOT NULL,
                segments_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    pub fn save_project(&self, result: &TranscriptionResult) -> Result<()> {
        let segments_json = serde_json::to_string(&result.segments)?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT OR REPLACE INTO projects
             (id, filename, engine, language, duration_ms, segment_count, segments_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                result.project_id,
                result.filename,
                result.engine,
                Option::<String>::None,
                result.duration_ms,
                result.segments.len() as i64,
                segments_json,
                result.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_projects(&self, limit: usize) -> Result<Vec<ProjectSummary>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, filename, engine, segment_count, duration_ms, created_at
             FROM projects ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit as i64], |row| {
            Ok(ProjectSummary {
                id: row.get(0)?,
                filename: row.get(1)?,
                engine: row.get(2)?,
                segment_count: row.get(3)?,
                duration_ms: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }
        Ok(projects)
    }

    pub fn get_project(&self, id: &str) -> Result<Option<TranscriptionResult>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, filename, engine, duration_ms, segments_json, created_at
             FROM projects WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let segments_json: String = row.get(4)?;
        Ok(Some(TranscriptionResult {
            project_id: row.get(0)?,
            filename: row.get(1)?,
            engine: row.get(2)?,
            duration_ms: row.get(3)?,
            segments: serde_json::from_str(&segments_json)?,
            created_at: row.get(5)?,
        }))
    }

    pub fn update_segments(&self, id: &str, segments: &[Segment]) -> Result<()> {
        let segments_json = serde_json::to_string(segments)?;
        let connection = self.connect()?;
        connection.execute(
            "UPDATE projects SET segments_json = ?1, segment_count = ?2 WHERE id = ?3",
            params![segments_json, segments.len() as i64, id],
        )?;
        Ok(())
    }

    pub fn delete_project(&self, id: &str) -> Result<()> {
        let connection = self.connect()?;
        connection.execute("DELETE FROM projects WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_result() -> TranscriptionResult {
        TranscriptionResult {
            project_id: "project-1".into(),
            filename: "demo.mp3".into(),
            engine: "openai".into(),
            duration_ms: 1_200,
            segments: vec![Segment::new(0, 1_200, "你好")],
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn saves_lists_and_loads_projects() {
        let directory = tempfile::tempdir().unwrap();
        let store = HistoryStore::open(directory.path().join("history.db")).unwrap();
        let result = sample_result();
        store.save_project(&result).unwrap();

        let projects = store.list_projects(10).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].filename, "demo.mp3");
        assert_eq!(projects[0].segment_count, 1);

        let loaded = store.get_project("project-1").unwrap().unwrap();
        assert_eq!(loaded.segments, result.segments);
    }

    #[test]
    fn updates_and_deletes_segments() {
        let directory = tempfile::tempdir().unwrap();
        let store = HistoryStore::open(directory.path().join("history.db")).unwrap();
        store.save_project(&sample_result()).unwrap();

        store
            .update_segments("project-1", &[Segment::new(0, 900, "更新")])
            .unwrap();
        let loaded = store.get_project("project-1").unwrap().unwrap();
        assert_eq!(loaded.segments[0].text, "更新");

        store.delete_project("project-1").unwrap();
        assert!(store.get_project("project-1").unwrap().is_none());
    }
}
