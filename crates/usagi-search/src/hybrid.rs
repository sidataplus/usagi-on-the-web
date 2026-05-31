use std::collections::HashMap;

use usagi_contracts::search::SearchResult;

#[derive(Debug, Clone, Copy)]
pub struct RrfOptions {
    pub rrf_k: f64,
    pub limit: usize,
}

#[derive(Debug, Clone)]
struct Candidate {
    result: SearchResult,
    lexical_rank: Option<usize>,
    dense_rank: Option<usize>,
}

pub fn fuse_rrf(
    lexical_results: Vec<SearchResult>,
    dense_results: Vec<SearchResult>,
    options: RrfOptions,
) -> Vec<SearchResult> {
    let mut candidates: HashMap<i64, Candidate> = HashMap::new();

    for item in lexical_results {
        candidates.insert(
            item.concept.concept_id,
            Candidate {
                lexical_rank: Some(item.rank),
                dense_rank: None,
                result: item,
            },
        );
    }

    for item in dense_results {
        candidates
            .entry(item.concept.concept_id)
            .and_modify(|candidate| {
                candidate.dense_rank = Some(item.rank);
            })
            .or_insert_with(|| Candidate {
                lexical_rank: None,
                dense_rank: Some(item.rank),
                result: item,
            });
    }

    let mut scored: Vec<(f64, Candidate)> = candidates
        .into_values()
        .map(|candidate| {
            (
                rrf_score(candidate.lexical_rank, candidate.dense_rank, options.rrf_k),
                candidate,
            )
        })
        .collect();

    scored.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .partial_cmp(left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| rank_sort_key(left.lexical_rank).cmp(&rank_sort_key(right.lexical_rank)))
            .then_with(|| rank_sort_key(left.dense_rank).cmp(&rank_sort_key(right.dense_rank)))
            .then_with(|| {
                left.result
                    .concept
                    .concept_id
                    .cmp(&right.result.concept.concept_id)
            })
    });

    scored
        .into_iter()
        .take(options.limit)
        .enumerate()
        .map(|(idx, (score, candidate))| {
            let mut result = candidate.result;
            result.rank = idx + 1;
            result.scores = serde_json::json!({
                "tantivy": null,
                "sapbert": null,
                "rrf": score
            });
            result.component_ranks = serde_json::json!({
                "tantivy": candidate.lexical_rank,
                "sapbert": candidate.dense_rank
            });
            result.method = "hybrid_rrf".to_string();
            result
        })
        .collect()
}

fn rrf_score(lexical_rank: Option<usize>, dense_rank: Option<usize>, rrf_k: f64) -> f64 {
    reciprocal_rank(lexical_rank, rrf_k) + reciprocal_rank(dense_rank, rrf_k)
}

fn reciprocal_rank(rank: Option<usize>, rrf_k: f64) -> f64 {
    rank.map(|rank| 1.0 / (rrf_k + rank as f64)).unwrap_or(0.0)
}

fn rank_sort_key(rank: Option<usize>) -> usize {
    rank.unwrap_or(usize::MAX)
}
