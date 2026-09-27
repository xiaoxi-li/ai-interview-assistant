//! SQLite 存储层：连接配置、迁移管理与 sqlite-vec 扩展加载。
//!
//! 设计要点：
//! * 单一数据库文件承载业务表 + FTS5 全文索引 + 向量索引，无独立服务进程。
//! * 迁移通过 `PRAGMA user_version` 递增应用，SQL 通过 `include_str!` 内嵌，
//!   保证发布产物无需附带迁移文件。
//! * sqlite-vec 以静态 auto-extension 方式注册（进程级仅一次），避免运行时加载 DLL。

use rusqlite::{ffi::sqlite3_auto_extension, Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex, Once};

/// 嵌入向量维度（需与最新迁移中 `vec_chunks` 声明一致）
pub const EMBEDDING_DIM: usize = 1024;

/// 当前 schema 版本（最后一条迁移的版本号）
pub const SCHEMA_VERSION: i64 = 2;

/// 触发嵌入向量全量重索引的迁移版本（维度变更 384 → 1024）
pub const EMBEDDING_DIM_MIGRATION_VERSION: i64 = 2;

/// 迁移列表：(目标版本号, SQL 内容)
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/001_init.sql")),
    (2, include_str!("../../migrations/002_embedding_1024.sql")),
];

/// 注册 sqlite-vec 扩展（进程级，只需一次）
pub fn register_vec_extension() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

/// 将 f32 向量编码为 sqlite-vec 所需的 little-endian blob
pub fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

/// 解码 sqlite-vec 返回的 blob 为 f32 向量
pub fn blob_to_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// 数据库句柄
pub struct Db {
    conn: Connection,
    /// 本次打开时是否执行了嵌入维度迁移（需触发全量向量重索引）
    embedding_rebuilt: bool,
}

impl Db {
    /// 打开内存库（主要供测试使用）
    pub fn open_in_memory() -> Result<Self> {
        register_vec_extension();
        let mut db = Self {
            conn: Connection::open_in_memory()?,
            embedding_rebuilt: false,
        };
        db.configure()?;
        db.migrate()?;
        Ok(db)
    }

    /// 打开（或创建）指定路径的数据库文件
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        register_vec_extension();
        let path = path.as_ref();
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            }
        }
        let mut db = Self {
            conn: Connection::open(path)?,
            embedding_rebuilt: false,
        };
        db.configure()?;
        db.migrate()?;
        Ok(db)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// 转为可共享的连接句柄（供并发组件如 VectorStore 使用）
    pub fn into_shared_conn(self) -> Arc<Mutex<Connection>> {
        Arc::new(Mutex::new(self.conn))
    }

    /// 当前 schema 版本
    pub fn schema_version(&self) -> Result<i64> {
        self.conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
    }

    /// 本次打开是否执行了嵌入维度迁移（调用方据此触发全量向量重索引）
    pub fn did_rebuild_embedding(&self) -> bool {
        self.embedding_rebuilt
    }

    /// 连接级配置
    fn configure(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;",
        )?;
        // 文件库启用 WAL 提升并发读写；内存库会返回 "memory"，不视为错误
        self.conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        Ok(())
    }

    /// 按版本号顺序应用未执行的迁移
    fn migrate(&mut self) -> Result<()> {
        let current = self.schema_version()?;
        for (version, sql) in MIGRATIONS {
            if *version > current {
                let tx = self.conn.transaction()?;
                tx.execute_batch(sql)?;
                tx.pragma_update(None, "user_version", version)?;
                tx.commit()?;
                if *version == EMBEDDING_DIM_MIGRATION_VERSION {
                    self.embedding_rebuilt = true;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn mem() -> Db {
        Db::open_in_memory().expect("open in-memory db")
    }

    fn table_names(db: &Db) -> Vec<String> {
        let mut stmt = db
            .conn()
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .expect("prepare");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query");
        rows.map(|r| r.expect("row")).collect()
    }

    #[test]
    fn migrations_create_core_tables() {
        let db = mem();
        let names = table_names(&db);
        let expected = [
            "groups",
            "notes",
            "tags",
            "note_tags",
            "note_versions",
            "chunks",
            "fts_index",
            "vec_chunks",
            "sessions",
            "transcript_segments",
            "qa_pairs",
            "settings",
        ];
        for t in expected {
            assert!(names.iter().any(|n| n == t), "缺少表：{t}");
        }
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut db = mem();
        db.migrate().expect("second migrate");
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn foreign_keys_enforced() {
        let db = mem();
        let err = db.conn().execute(
            "INSERT INTO notes (id, title, content, group_id, created_at, updated_at)
             VALUES ('n1', 't', 'c', 'not-exist', 0, 0)",
            [],
        );
        assert!(err.is_err(), "外键约束应阻止无效 group_id");
    }

    #[test]
    fn fts5_is_available_and_searchable() {
        let db = mem();
        // 模拟 jieba 预分词结果：以空格分隔的中文词
        db.conn()
            .execute(
                "INSERT INTO fts_index (note_id, title, content, tags)
                 VALUES (?1, ?2, ?3, ?4)",
                params!["n1", "项目 架构", "使用 Rust 与 前端 框架 构建", "架构"],
            )
            .expect("insert fts row");

        let hit: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM fts_index WHERE fts_index MATCH '架构'",
                [],
                |row| row.get(0),
            )
            .expect("fts query");
        assert_eq!(hit, 1);

        let miss: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM fts_index WHERE fts_index MATCH '数据库'",
                [],
                |row| row.get(0),
            )
            .expect("fts query miss");
        assert_eq!(miss, 0);
    }

    #[test]
    fn sqlite_vec_extension_is_loaded() {
        let db = mem();
        let version: String = db
            .conn()
            .query_row("SELECT vec_version()", [], |row| row.get(0))
            .expect("vec_version()");
        assert!(!version.trim().is_empty(), "sqlite-vec 未正确加载");
    }

    #[test]
    fn vec_chunks_supports_knn_search() {
        let db = mem();
        let basis = |idx: usize| {
            let mut v = vec![0.0f32; EMBEDDING_DIM];
            v[idx] = 1.0;
            v
        };

        for (id, idx) in [("c0", 0usize), ("c1", 1), ("c2", 2)] {
            db.conn()
                .execute(
                    "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
                    params![id, vec_to_blob(&basis(idx))],
                )
                .expect("insert vec row");
        }

        let nearest: String = db
            .conn()
            .query_row(
                "SELECT chunk_id FROM vec_chunks
                 WHERE embedding MATCH ?1 ORDER BY distance LIMIT 1",
                params![vec_to_blob(&basis(1))],
                |row| row.get(0),
            )
            .expect("knn query");
        assert_eq!(nearest, "c1");
    }

    #[test]
    fn vec_blob_round_trip() {
        let v = vec![0.5f32, -1.25, 3.75];
        assert_eq!(blob_to_vec(&vec_to_blob(&v)), v);
    }
}
