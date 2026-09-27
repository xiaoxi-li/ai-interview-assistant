-- ============================================================
-- 002_embedding_1024.sql —— 嵌入维度 384 → 1024（切换 BGE-M3）
-- 说明：
--   * sqlite-vec 的 vec0 虚拟表不支持 ALTER，需 DROP 后重建；
--   * 旧 384 维向量随之清空，由应用层在迁移完成后触发全量向量重索引；
--   * chunks 表保留，重索引时一并重建。
-- ============================================================

DROP TABLE IF EXISTS vec_chunks;

CREATE VIRTUAL TABLE vec_chunks USING vec0(
    chunk_id TEXT PRIMARY KEY,
    embedding float[1024]
);
