//! 知识库领域：Markdown 导入、笔记与群组数据结构。

pub mod export;
pub mod markdown;
pub mod store;

use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 笔记数据传输对象
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub id: String,
    pub title: String,
    pub content: String,
    pub group_id: Option<String>,
    pub tags: Vec<String>,
    pub is_deleted: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 群组数据传输对象
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDto {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub order: i64,
}

/// 标签数据传输对象
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub id: String,
    pub name: String,
    pub color: String,
}

/// 导入结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    /// 成功导入的笔记数
    pub imported: usize,
    /// 跳过的文件及原因
    pub skipped: Vec<String>,
    /// 新建的笔记
    pub notes: Vec<NoteDto>,
}

/// 递归展开输入路径，收集全部 Markdown 文件（结果稳定排序）
pub fn collect_markdown_files(inputs: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack: Vec<PathBuf> = inputs.to_vec();

    while let Some(path) = stack.pop() {
        if path.is_dir() {
            let entries = match std::fs::read_dir(&path) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let child = entry.path();
                // 跳过隐藏目录（如 .git）与临时目录
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                stack.push(child);
            }
        } else if markdown::is_markdown_path(&path) {
            files.push(path);
        }
    }

    files.sort();
    files.dedup();
    files
}

fn read_and_parse(path: &PathBuf) -> std::io::Result<markdown::ParsedMarkdown> {
    let raw = std::fs::read_to_string(path)?;
    let fallback = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未命名笔记")
        .to_string();
    Ok(markdown::parse(&raw, &fallback))
}

/// 将 Markdown 文件导入知识库
pub fn import_paths(
    conn: &Connection,
    inputs: &[PathBuf],
    group_id: Option<&str>,
) -> Result<ImportOutcome> {
    let repo = store::NotesRepo::new(conn);
    let mut notes = Vec::new();
    let mut skipped = Vec::new();

    for path in collect_markdown_files(inputs) {
        match read_and_parse(&path) {
            Ok(parsed) => notes.push(repo.create(&parsed.title, &parsed.content, group_id)?),
            Err(err) => skipped.push(format!("{}: {err}", path.display())),
        }
    }

    Ok(ImportOutcome { imported: notes.len(), skipped, notes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;
    use store::NotesRepo;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "aia-test-{tag}-{}-{}",
            std::process::id(),
            store::now_ms()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn imports_single_file_with_h1_title() {
        let dir = temp_dir("single");
        let file = dir.join("note.md");
        std::fs::write(&file, "# 设计模式总结\n内容正文").expect("write file");

        let db = Db::open_in_memory().expect("db");
        let outcome = import_paths(db.conn(), &[file], None).expect("import");

        assert_eq!(outcome.imported, 1);
        assert!(outcome.skipped.is_empty());
        assert_eq!(outcome.notes[0].title, "设计模式总结");
        assert_eq!(NotesRepo::new(db.conn()).count().expect("count"), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn imports_directory_recursively_and_ignores_non_markdown() {
        let dir = temp_dir("bulk");
        std::fs::write(dir.join("a.md"), "# 笔记 A\n内容 A").expect("write a");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).expect("mkdir sub");
        std::fs::write(sub.join("b.markdown"), "## 笔记 B\n内容 B").expect("write b");
        std::fs::write(sub.join("ignore.txt"), "不是 Markdown").expect("write txt");

        let db = Db::open_in_memory().expect("db");
        let outcome = import_paths(db.conn(), &[dir.clone()], None).expect("import");

        assert_eq!(outcome.imported, 2, "应导入 2 个 Markdown 文件");
        let mut titles: Vec<String> = outcome.notes.iter().map(|n| n.title.clone()).collect();
        titles.sort();
        assert_eq!(titles, vec!["笔记 A".to_string(), "笔记 B".to_string()]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn title_falls_back_to_file_name() {
        let dir = temp_dir("fallback");
        let file = dir.join("未命名标题.md");
        std::fs::write(&file, "没有标题的正文").expect("write file");

        let db = Db::open_in_memory().expect("db");
        let outcome = import_paths(db.conn(), &[file], None).expect("import");
        assert_eq!(outcome.notes[0].title, "未命名标题");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_path_is_ignored_gracefully() {
        let db = Db::open_in_memory().expect("db");
        let outcome = import_paths(
            db.conn(),
            &[PathBuf::from("D:/definitely-not-exists/xyz.md")],
            None,
        )
        .expect("import");
        assert_eq!(outcome.imported, 0);
    }

    #[test]
    fn collect_ignores_hidden_directories() {
        let dir = temp_dir("hidden");
        let hidden = dir.join(".git");
        std::fs::create_dir_all(&hidden).expect("mkdir hidden");
        std::fs::write(hidden.join("hooks.md"), "# 不应被导入").expect("write hidden");
        std::fs::write(dir.join("visible.md"), "# 可见").expect("write visible");

        let files = collect_markdown_files(&[dir.clone()]);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("visible.md"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
