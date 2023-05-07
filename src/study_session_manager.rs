//! Contains utilities to use study sessions saved by the user.
//!
//! Trane's default mode for scheduling exercises is to traverse the entire graph. Study sessions
//! allow students to traverse specific parts of the graph for the specified amount of time. This
//! module allows them to re-use study sessions they have previously saved.

use anyhow::{bail, Context, Result};
use std::{collections::HashMap, fs::File, io::BufReader};

use crate::data::filter::StudySession;

/// A trait with functions to manage saved study session. Each session is given a unique name to use
/// as an identifier.
pub trait StudySessionManager {
    /// Gets the study session with the given ID.
    fn get_study_session(&self, id: &str) -> Option<StudySession>;

    /// Returns a list of study session IDs and descriptions.
    fn list_study_sessions(&self) -> Vec<(String, String)>;
}

/// An implementation of [StudySessionManager] backed by the local file system.
pub(crate) struct LocalStudySessionManager {
    /// A map of session IDs to sessions.
    pub sessions: HashMap<String, StudySession>,
}

impl LocalStudySessionManager {
    /// Scans all study sessions in the given directory and returns a map of study sessions.
    fn scan_sessions(session_directory: &str) -> Result<HashMap<String, StudySession>> {
        let mut sessions = HashMap::new();
        for entry in std::fs::read_dir(session_directory).with_context(|| {
            format!("Failed to read study session directory {session_directory}")
        })? {
            // Try to read the file as a [StudySession].
            let entry =
                entry.with_context(|| "Failed to read file entry for saved study session")?;
            let file = File::open(entry.path()).with_context(|| {
                format!(
                    "Failed to open saved study session file {}",
                    entry.path().display()
                )
            })?;
            let reader = BufReader::new(file);
            let session: StudySession = serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to parse study session from {}",
                    entry.path().display()
                )
            })?;

            // Check for duplicate IDs before inserting the study session..
            if sessions.contains_key(&session.id) {
                bail!("Found multiple study sessions with ID {}", session.id);
            }
            sessions.insert(session.id.clone(), session);
        }
        Ok(sessions)
    }

    /// Creates a new `LocalStudySessionManager`.
    pub fn new(session_directory: &str) -> Result<LocalStudySessionManager> {
        Ok(LocalStudySessionManager {
            sessions: LocalStudySessionManager::scan_sessions(session_directory)?,
        })
    }
}

impl StudySessionManager for LocalStudySessionManager {
    fn get_study_session(&self, id: &str) -> Option<StudySession> {
        self.sessions.get(id).cloned()
    }

    fn list_study_sessions(&self) -> Vec<(String, String)> {
        // Create a list of (ID, description) pairs.
        let mut sessions: Vec<(String, String)> = self
            .sessions
            .iter()
            .map(|(id, session)| (id.clone(), session.description.clone()))
            .collect();

        // Sort the session by their IDs.
        sessions.sort_by(|a, b| a.0.cmp(&b.0));
        sessions
    }
}

#[cfg(test)]
mod test {
    use anyhow::{Ok, Result};
    use std::{os::unix::prelude::PermissionsExt, path::Path};
    use tempfile::TempDir;

    use crate::{
        data::filter::StudySession,
        study_session_manager::{LocalStudySessionManager, StudySessionManager},
    };

    /// Creates some study sessions for testing.
    fn test_sessions() -> Vec<StudySession> {
        vec![
            StudySession {
                id: "session1".into(),
                description: "Session 1".into(),
                parts: vec![],
            },
            StudySession {
                id: "session2".into(),
                description: "Session 2".into(),
                parts: vec![],
            },
        ]
    }

    /// Writes the sessions to the given directory.
    fn write_sessions(sessions: Vec<StudySession>, dir: &Path) -> Result<()> {
        for session in sessions {
            // Give each file a unique name.
            let timestamp_ns = chrono::Utc::now().timestamp_nanos();
            let session_path = dir.join(format!("{}_{}.json", session.id, timestamp_ns));
            let session_json = serde_json::to_string(&session)?;
            std::fs::write(session_path, session_json)?;
        }
        Ok(())
    }

    /// Verifies creating a study session manager with valid sessions.
    #[test]
    fn session_manager() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let sessions = test_sessions();
        write_sessions(sessions.clone(), temp_dir.path())?;
        let manager = LocalStudySessionManager::new(temp_dir.path().to_str().unwrap())?;

        let session_list = manager.list_study_sessions();
        assert_eq!(
            session_list,
            vec![
                ("session1".to_string(), "Session 1".to_string()),
                ("session2".to_string(), "Session 2".to_string())
            ]
        );

        for (index, (id, _)) in session_list.iter().enumerate() {
            let session = manager.get_study_session(&id);
            assert!(session.is_some());
            let session = session.unwrap();
            assert_eq!(sessions[index], session);
        }
        Ok(())
    }

    /// Verifies that sessions with repeated IDs cause the study session manager to fail.
    #[test]
    fn sessions_repeated_ids() -> Result<()> {
        let sessions = vec![
            StudySession {
                id: "session1".into(),
                description: "Session 1".into(),
                parts: vec![],
            },
            StudySession {
                id: "session1".into(),
                description: "Session 2".into(),
                parts: vec![],
            },
        ];

        let temp_dir = TempDir::new()?;
        write_sessions(sessions.clone(), temp_dir.path())?;
        assert!(LocalStudySessionManager::new(temp_dir.path().to_str().unwrap()).is_err());
        Ok(())
    }

    /// Verifies that trying to read study sessions from an invalid directory fails.
    #[test]
    fn read_bad_directory() -> Result<()> {
        assert!(LocalStudySessionManager::new("bad_directory").is_err());
        Ok(())
    }

    /// Verifies that study sessions in an invalid format cause the study session manager to fail.
    #[test]
    fn read_bad_file_format() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let bad_file = temp_dir.path().join("bad_file.json");
        std::fs::write(bad_file, "bad json")?;
        assert!(LocalStudySessionManager::new(temp_dir.path().to_str().unwrap()).is_err());
        Ok(())
    }

    /// Verifies that sessions with bad permissions cause the study session manager to fail.
    #[test]
    fn read_bad_file_permissions() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let bad_file = temp_dir.path().join("bad_file.json");
        std::fs::write(bad_file.clone(), "bad json")?;
        std::fs::set_permissions(bad_file, std::fs::Permissions::from_mode(0o000))?;
        assert!(LocalStudySessionManager::new(temp_dir.path().to_str().unwrap()).is_err());
        Ok(())
    }
}
