//! VectorStore：向量检索抽象 + 基于 sqlite-vec 的默认实现。
//!
//! 抽象的目的是在未来数据规模增长时（如 >50 万块）可无缝替换为
//! LanceDB / Qdrant 等实现，而不影响上层检索与 RRF 融合逻辑。

use super::types::{ProviderError, ProviderResult, VectorHit};
use crate::storage::{vec_to_blob, EMBEDDING_DIM};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex, MutexGuard};

/// 向量存储抽象
pub trait VectorStore: Send + Sync {
    /// 写入/覆盖一个分块向量
    fn upsert(&self, chunk_id: &str, embedding: &[f32]) -> ProviderResult<()>;

    /// 相似度检索（按距离升序）
    fn search(&self, query: &[f32], top_k: usize) -> ProviderResult<Vec<VectorHit>>;

    /// 删除单个分块向量
    fn remove(&self, chunk_id: &str) -> ProviderResult<()>;

    /// 当前向量总数
    fn len(&self) -> ProviderResult<usize>;

    fn is_empty(&self) -> ProviderResult<bool> {
        Ok(self.len()? == 0)
    }
}

/// 基于 sqlite-vec（vec0 虚拟表）的实现
pub struct SqliteVecStore {
    conn: Arc<Mutex<Connection>>,
    dim: usize,
}

impl SqliteVecStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn, dim: EMBEDDING_DIM }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    fn lock(&self) -> ProviderResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| ProviderError::Other(format!("连接锁已中毒：{e}")))
    }

    fn check_dim(&self, v: &[f32]) -> ProviderResult<()> {
        if v.len() != self.dim {
            return Err(ProviderError::InvalidResponse(format!(
                "向量维度不匹配：期望 {}，实际 {}",
                self.dim,
                v.len()
            )));
        }
        Ok(())
    }
}

impl VectorStore for SqliteVecStore {
    fn upsert(&self, chunk_id: &str, embedding: &[f32]) -> ProviderResult<()> {
        self.check_dim(embedding)?;
        let conn = self.lock()?;
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM vec_chunks WHERE chunk_id = ?1", params![chunk_id])?;
        tx.execute(
            "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
            params![chunk_id, vec_to_blob(embedding)],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn search(&self, query: &[f32], top_k: usize) -> ProviderResult<Vec<VectorHit>> {
        self.check_dim(query)?;
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT chunk_id, distance FROM vec_chunks
             WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![vec_to_blob(query), top_k as i64], |row| {
            Ok(VectorHit { chunk_id: row.get(0)?, distance: row.get(1)? })
        })?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        Ok(hits)
    }

    fn remove(&self, chunk_id: &str) -> ProviderResult<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM vec_chunks WHERE chunk_id = ?1", params![chunk_id])?;
        Ok(())
    }

    fn len(&self) -> ProviderResult<usize> {
        let conn = self.lock()?;
        let count: i64 =
            conn.query_row("SELECT count(*) FROM vec_chunks", [], |row| row.get(0))?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;

    fn basis(idx: usize, dim: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        v[idx] = 1.0;
        v
    }

    fn store() -> SqliteVecStore {
        let db = Db::open_in_memory().expect("db");
        SqliteVecStore::new(db.into_shared_conn())
    }

    #[test]
    fn upsert_then_search_returns_nearest() {
        let s = store();
        s.upsert("c0", &basis(0, EMBEDDING_DIM)).expect("upsert c0");
        s.upsert("c1", &basis(1, EMBEDDING_DIM)).expect("upsert c1");
        s.upsert("c2", &basis(2, EMBEDDING_DIM)).expect("upsert c2");

        let hits = s.search(&basis(1, EMBEDDING_DIM), 2).expect("search");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].chunk_id, "c1");
        assert!(hits[0].distance <= hits[1].distance);
    }

    #[test]
    fn upsert_is_idempotent_for_same_chunk() {
        let s = store();
        s.upsert("c0", &basis(0, EMBEDDING_DIM)).expect("first");
        s.upsert("c0", &basis(3, EMBEDDING_DIM)).expect("second");
        assert_eq!(s.len().expect("len"), 1);

        // 覆盖后应命中新的方向
        let hits = s.search(&basis(3, EMBEDDING_DIM), 1).expect("search");
        assert_eq!(hits[0].chunk_id, "c0");
    }

    #[test]
    fn remove_deletes_vector() {
        let s = store();
        s.upsert("c0", &basis(0, EMBEDDING_DIM)).expect("upsert");
        assert_eq!(s.len().expect("len"), 1);
        s.remove("c0").expect("remove");
        assert_eq!(s.len().expect("len"), 0);
        assert!(s.is_empty().expect("is_empty"));
    }

    #[test]
    fn dimension_mismatch_is_rejected() {
        let s = store();
        let wrong = vec![0.5f32; 8];
        assert!(s.upsert("bad", &wrong).is_err());
        assert!(s.search(&wrong, 1).is_err());
    }

    #[test]
    fn empty_store_returns_no_hits() {
        let s = store();
        assert!(s.is_empty().expect("is_empty"));
        let hits = s.search(&basis(0, EMBEDDING_DIM), 5).expect("search");
        assert!(hits.is_empty());
    }
}
