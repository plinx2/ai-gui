use async_trait::async_trait;

use crate::error::Result;

#[async_trait]
pub trait KnowledgeStore: Send + Sync {
    /// ユーザー入力に関連するナレッジを取得して文字列として返す。
    /// 結果はユーザーメッセージの前に付加される。
    async fn retrieve(&self, query: &str) -> Result<String>;
}
