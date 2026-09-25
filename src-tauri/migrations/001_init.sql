-- ============================================================
-- 001_init.sql —— 初始 schema
-- 说明：
--   * 所有主键为 TEXT（ULID/UUID 由应用层生成）
--   * 时间戳统一为毫秒级 Unix epoch（INTEGER）
--   * fts_index 与 vec_chunks 为虚拟表，由应用层显式同步
-- ============================================================

-- ---------- 知识库：群组 ----------
CREATE TABLE IF NOT EXISTS groups (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    parent_id   TEXT REFERENCES groups(id) ON DELETE CASCADE,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_groups_parent ON groups(parent_id);

-- ---------- 知识库：笔记 ----------
CREATE TABLE IF NOT EXISTS notes (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL DEFAULT '',
    content     TEXT NOT NULL DEFAULT '',
    group_id    TEXT REFERENCES groups(id) ON DELETE SET NULL,
    is_deleted  INTEGER NOT NULL DEFAULT 0,
    deleted_at  INTEGER,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_notes_group ON notes(group_id);
CREATE INDEX IF NOT EXISTS idx_notes_deleted ON notes(is_deleted);
CREATE INDEX IF NOT EXISTS idx_notes_updated ON notes(updated_at DESC);

-- ---------- 知识库：标签 ----------
CREATE TABLE IF NOT EXISTS tags (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    color       TEXT NOT NULL DEFAULT '#4F6BED',
    created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS note_tags (
    note_id     TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id      TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (note_id, tag_id)
);
CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag_id);

-- ---------- 知识库：版本历史 ----------
CREATE TABLE IF NOT EXISTS note_versions (
    id          TEXT PRIMARY KEY,
    note_id     TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    content     TEXT NOT NULL,
    title       TEXT NOT NULL DEFAULT '',
    version     INTEGER NOT NULL,
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_note_versions_note ON note_versions(note_id, version DESC);

-- ---------- 检索：分块 ----------
CREATE TABLE IF NOT EXISTS chunks (
    id          TEXT PRIMARY KEY,
    note_id     TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    content     TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chunks_note ON chunks(note_id, position);

-- ---------- 检索：全文索引（FTS5）----------
-- title/content 写入前需经 jieba 预分词并以空格拼接（unicode61 对中文无效）
CREATE VIRTUAL TABLE IF NOT EXISTS fts_index USING fts5(
    note_id UNINDEXED,
    title,
    content,
    tags,
    tokenize = 'unicode61'
);

-- ---------- 检索：向量索引（sqlite-vec）----------
-- 维度与 storage::EMBEDDING_DIM 保持一致；变更维度需新增迁移
CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
    chunk_id TEXT PRIMARY KEY,
    embedding float[384]
);

-- ---------- 面试会话 ----------
CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL DEFAULT '',
    started_at  INTEGER NOT NULL,
    ended_at    INTEGER
);
CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at DESC);

CREATE TABLE IF NOT EXISTS transcript_segments (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    speaker     TEXT NOT NULL CHECK (speaker IN ('me', 'interviewer')),
    text        TEXT NOT NULL,
    is_final    INTEGER NOT NULL DEFAULT 1,
    ts          INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_segments_session ON transcript_segments(session_id, ts);

CREATE TABLE IF NOT EXISTS qa_pairs (
    id            TEXT PRIMARY KEY,
    session_id    TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    question      TEXT NOT NULL,
    answer        TEXT NOT NULL DEFAULT '',
    sources_json  TEXT NOT NULL DEFAULT '[]',
    ts            INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_qa_session ON qa_pairs(session_id, ts);

-- ---------- 配置 ----------
CREATE TABLE IF NOT EXISTS settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  INTEGER NOT NULL
);
