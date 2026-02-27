use async_trait::async_trait;

use crate::agent::session::Session;
use crate::error::Result;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn save(&self, session: &Session) -> Result<()>;
    async fn load(&self, session_id: &str) -> Result<Option<Session>>;
    async fn load_all(&self) -> Result<Vec<Session>>;
    async fn delete(&self, session_id: &str) -> Result<()>;
}
