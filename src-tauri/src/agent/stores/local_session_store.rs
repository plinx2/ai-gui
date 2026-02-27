use std::path::PathBuf;

use async_trait::async_trait;
use tokio::fs;

use crate::agent::session::Session;
use crate::agent::session_store::SessionStore;
use crate::error::Result;

pub struct LocalSessionStore {
    base_dir: PathBuf,
}

impl LocalSessionStore {
    pub async fn new(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir).await?;
        Ok(Self { base_dir })
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(format!("{}.json", session_id))
    }
}

#[async_trait]
impl SessionStore for LocalSessionStore {
    async fn save(&self, session: &Session) -> Result<()> {
        let path = self.session_path(&session.id);
        let json = serde_json::to_string_pretty(session)?;
        fs::write(path, json).await?;
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Option<Session>> {
        let path = self.session_path(session_id);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path).await?;
        let session = serde_json::from_slice(&bytes)?;
        Ok(Some(session))
    }

    async fn load_all(&self) -> Result<Vec<Session>> {
        let mut sessions = Vec::new();
        let mut entries = fs::read_dir(&self.base_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let bytes = fs::read(&path).await?;
                match serde_json::from_slice::<Session>(&bytes) {
                    Ok(session) => sessions.push(session),
                    Err(e) => {
                        eprintln!("Failed to parse session {:?}: {}", path, e);
                    }
                }
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    async fn delete(&self, session_id: &str) -> Result<()> {
        let path = self.session_path(session_id);
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }
}
