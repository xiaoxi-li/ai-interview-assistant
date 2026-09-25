//! RRF（Reciprocal Rank Fusion）融合排序。
//!
//! 将多路召回的有序结果按 `1 / (k + rank)` 累加得分：
//! * 只依赖名次，无需跨路归一化分数，天然抗量纲差异；
//! * 同时命中多路的条目会获得叠加分数，从而排到更前。

use std::collections::HashMap;

/// RRF 平滑常数（业界常用 60）
pub const RRF_K: f32 = 60.0;

/// 融合多路有序召回结果，返回按融合得分降序的 (id, score)
pub fn rrf_fuse(ranked_lists: &[Vec<String>], k: f32) -> Vec<(String, f32)> {
    let mut fused: Vec<(String, f32)> = Vec::new();
    let mut position: HashMap<&str, usize> = HashMap::new();

    for list in ranked_lists {
        for (index, id) in list.iter().enumerate() {
            let contribution = 1.0 / (k + index as f32 + 1.0);
            match position.get(id.as_str()) {
                Some(&slot) => fused[slot].1 += contribution,
                None => {
                    position.insert(id.as_str(), fused.len());
                    fused.push((id.clone(), contribution));
                }
            }
        }
    }

    fused.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0)) // 得分相同时按 ID 稳定排序
    });
    fused
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(entries: &[(String, f32)]) -> Vec<String> {
        entries.iter().map(|(id, _)| id.clone()).collect()
    }

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn single_list_preserves_order() {
        let fused = rrf_fuse(&[list(&["a", "b", "c"])], RRF_K);
        assert_eq!(ids(&fused), vec!["a", "b", "c"]);
        assert!(fused[0].1 > fused[1].1);
    }

    #[test]
    fn items_in_both_lists_rank_higher() {
        let fts = list(&["a", "b", "c"]);
        let vector = list(&["c", "d"]);

        let fused = rrf_fuse(&[fts, vector], RRF_K);
        // c 同时被两路召回，应超过仅被单路召回的 a
        assert_eq!(ids(&fused)[0], "c");
    }

    #[test]
    fn score_is_sum_of_reciprocal_ranks() {
        let fused = rrf_fuse(&[list(&["a"]), list(&["a"])], 60.0);
        let expected = 1.0 / 61.0 + 1.0 / 61.0;
        assert!((fused[0].1 - expected).abs() < 1e-6);
    }

    #[test]
    fn duplicate_ids_within_one_list_are_counted_once_per_position() {
        let fused = rrf_fuse(&[list(&["a", "a"])], RRF_K);
        let expected = 1.0 / 61.0 + 1.0 / 62.0;
        assert_eq!(fused.len(), 1);
        assert!((fused[0].1 - expected).abs() < 1e-6);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert!(rrf_fuse(&[], RRF_K).is_empty());
        assert!(rrf_fuse(&[vec![], vec![]], RRF_K).is_empty());
    }

    #[test]
    fn ties_are_broken_by_id_for_stability() {
        let fused = rrf_fuse(&[list(&["b"]), list(&["a"])], RRF_K);
        assert_eq!(ids(&fused), vec!["a", "b"], "同分按 ID 升序，保证结果稳定");
    }
}
