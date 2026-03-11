use std::path::PathBuf;

use async_trait::async_trait;
use tokio::fs;

use crate::agent::playbook::{Playbook, PlaybookStore};
use crate::error::Result;

pub struct LocalPlaybookStore {
    base_dir: PathBuf,
}

impl LocalPlaybookStore {
    pub async fn new(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir).await?;
        Ok(Self { base_dir })
    }

    fn playbook_path(&self, id: &str) -> PathBuf {
        self.base_dir.join(format!("{}.json", id))
    }
}

#[async_trait]
impl PlaybookStore for LocalPlaybookStore {
    async fn load_all(&self) -> Result<Vec<Playbook>> {
        let mut playbooks = Vec::new();
        let mut entries = fs::read_dir(&self.base_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let bytes = fs::read(&path).await?;
                match serde_json::from_slice::<Playbook>(&bytes) {
                    Ok(p) => playbooks.push(p),
                    Err(e) => eprintln!("Failed to parse playbook {:?}: {}", path, e),
                }
            }
        }
        playbooks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(playbooks)
    }

    async fn save(&self, playbook: &Playbook) -> Result<()> {
        let path = self.playbook_path(&playbook.id);
        let json = serde_json::to_string_pretty(playbook)?;
        fs::write(path, json).await?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let path = self.playbook_path(id);
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }
}
