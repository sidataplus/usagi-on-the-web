use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TieBreakOptions {
    pub epsilon: f32,
    pub top_n: usize,
}

impl Default for TieBreakOptions {
    fn default() -> Self {
        Self {
            epsilon: 0.01,
            top_n: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TieBreakCandidate {
    pub concept_id: i64,
    pub concept_name: String,
    pub bimaxsim: f32,
    pub tachiom_maxsim: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrugCueFeatures {
    pub strength_exact: Option<bool>,
    pub dose_form_match: Option<bool>,
    pub route_match: Option<bool>,
    pub release_match: Option<bool>,
    pub brand_match: Option<bool>,
    pub combination_count_match: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedCandidate {
    pub concept_id: i64,
    pub concept_name: String,
    pub bimaxsim: f32,
    pub tachiom_maxsim: f32,
    pub tie_breaker: f32,
    pub final_score: f32,
    pub features: DrugCueFeatures,
}

pub fn rank_near_ties(
    source_name: &str,
    candidates: Vec<TieBreakCandidate>,
    options: TieBreakOptions,
) -> Vec<RankedCandidate> {
    let query_cues = DrugCues::extract(source_name);
    let mut ranked: Vec<RankedCandidate> = candidates
        .into_iter()
        .map(|candidate| {
            let candidate_cues = DrugCues::extract(&candidate.concept_name);
            let (tie_breaker, features) = score_cues(&query_cues, &candidate_cues);
            RankedCandidate {
                concept_id: candidate.concept_id,
                concept_name: candidate.concept_name,
                bimaxsim: candidate.bimaxsim,
                tachiom_maxsim: candidate.tachiom_maxsim,
                tie_breaker,
                final_score: candidate.bimaxsim,
                features,
            }
        })
        .collect();

    ranked.sort_by(base_order);
    let Some(anchor) = ranked.first().map(|candidate| candidate.bimaxsim) else {
        return ranked;
    };
    let split = ranked
        .iter()
        .take(options.top_n)
        .take_while(|candidate| anchor - candidate.bimaxsim <= options.epsilon)
        .count();
    ranked[..split].sort_by(tie_break_order);
    ranked
}

fn base_order(left: &RankedCandidate, right: &RankedCandidate) -> std::cmp::Ordering {
    right
        .bimaxsim
        .total_cmp(&left.bimaxsim)
        .then_with(|| right.tachiom_maxsim.total_cmp(&left.tachiom_maxsim))
        .then_with(|| left.concept_id.cmp(&right.concept_id))
}

fn tie_break_order(left: &RankedCandidate, right: &RankedCandidate) -> std::cmp::Ordering {
    right
        .tie_breaker
        .total_cmp(&left.tie_breaker)
        .then_with(|| base_order(left, right))
}

fn score_cues(query: &DrugCues, candidate: &DrugCues) -> (f32, DrugCueFeatures) {
    let strength_exact = compare_sets(&query.strengths, &candidate.strengths);
    let dose_form_match = compare_sets(&query.forms, &candidate.forms);
    let route_match = compare_sets(&query.routes, &candidate.routes);
    let release_match = compare_sets(&query.releases, &candidate.releases);
    let combination_count_match = Some(query.combination_count == candidate.combination_count);

    let mut score = 0.0;
    score += bool_score(strength_exact, 0.030, -0.030);
    score += bool_score(dose_form_match, 0.012, -0.012);
    score += bool_score(route_match, 0.006, -0.006);
    score += bool_score(release_match, 0.006, -0.006);
    score += bool_score(combination_count_match, 0.010, -0.010);

    (
        score,
        DrugCueFeatures {
            strength_exact,
            dose_form_match,
            route_match,
            release_match,
            brand_match: None,
            combination_count_match,
        },
    )
}

fn bool_score(value: Option<bool>, match_score: f32, mismatch_score: f32) -> f32 {
    match value {
        Some(true) => match_score,
        Some(false) => mismatch_score,
        None => 0.0,
    }
}

fn compare_sets(query_values: &[String], candidate_values: &[String]) -> Option<bool> {
    if query_values.is_empty() && candidate_values.is_empty() {
        return None;
    }
    Some(query_values == candidate_values)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DrugCues {
    strengths: Vec<String>,
    forms: Vec<String>,
    routes: Vec<String>,
    releases: Vec<String>,
    combination_count: usize,
}

impl DrugCues {
    fn extract(text: &str) -> Self {
        let normalized = normalize_text(text);
        let tokens: Vec<&str> = normalized.split_whitespace().collect();
        Self {
            strengths: extract_strengths(text, &tokens),
            forms: extract_members(&tokens, dose_form_map),
            routes: extract_members(&tokens, route_map),
            releases: extract_members(&tokens, release_map),
            combination_count: combination_count(text, &tokens),
        }
    }
}

fn normalize_text(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '%' {
                ch
            } else {
                ' '
            }
        })
        .collect()
}

fn extract_strengths(original: &str, tokens: &[&str]) -> Vec<String> {
    let mut strengths = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if let Some((amount, unit)) = split_amount_unit(token) {
            strengths.push(format!("{amount}{unit}"));
            continue;
        }
        if is_number(token) {
            if let Some(next) = tokens
                .get(index + 1)
                .and_then(|value| normalize_unit(value))
            {
                strengths.push(format!("{token}{next}"));
            }
        }
    }
    if strengths.is_empty() {
        strengths.extend(extract_unitless_slash_mg_strengths(original));
    }
    strengths.sort();
    strengths.dedup();
    strengths
}

fn extract_unitless_slash_mg_strengths(original: &str) -> Vec<String> {
    normalize_text_preserving_slash(original)
        .split_whitespace()
        .filter(|token| token.contains('/'))
        .flat_map(|token| {
            let parts: Vec<&str> = token.split('/').filter(|part| !part.is_empty()).collect();
            if parts.len() < 2 || !parts.iter().all(|part| is_number(part)) {
                return Vec::new();
            }

            parts
                .into_iter()
                .map(|amount| format!("{amount}mg"))
                .collect()
        })
        .collect()
}

fn normalize_text_preserving_slash(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '/' {
                ch
            } else {
                ' '
            }
        })
        .collect()
}

fn split_amount_unit(token: &str) -> Option<(&str, &'static str)> {
    let split_at = token
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_ascii_digit() && ch != '.').then_some(index))?;
    let (amount, unit) = token.split_at(split_at);
    if amount.is_empty() || !is_number(amount) {
        return None;
    }
    normalize_unit(unit).map(|normalized_unit| (amount, normalized_unit))
}

fn is_number(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
}

fn normalize_unit(value: &str) -> Option<&'static str> {
    match value {
        "mg" => Some("mg"),
        "mcg" | "ug" => Some("mcg"),
        "g" => Some("g"),
        "ml" => Some("ml"),
        "%" => Some("%"),
        "unit" | "units" | "iu" => Some("unit"),
        _ => None,
    }
}

fn extract_members(tokens: &[&str], map: fn(&str) -> Option<&'static str>) -> Vec<String> {
    let mut values: Vec<String> = tokens
        .iter()
        .filter_map(|token| map(token).map(ToString::to_string))
        .collect();
    values.sort();
    values.dedup();
    values
}

fn dose_form_map(token: &str) -> Option<&'static str> {
    match token {
        "tablet" | "tablets" | "tab" | "tabs" => Some("tablet"),
        "capsule" | "capsules" | "cap" | "caps" => Some("capsule"),
        "solution" => Some("solution"),
        "suspension" => Some("suspension"),
        "injection" | "injectable" => Some("injection"),
        "inhaler" => Some("inhaler"),
        "cream" => Some("cream"),
        "ointment" => Some("ointment"),
        "patch" => Some("patch"),
        _ => None,
    }
}

fn route_map(token: &str) -> Option<&'static str> {
    match token {
        "oral" | "po" => Some("oral"),
        "iv" | "intravenous" => Some("intravenous"),
        "topical" => Some("topical"),
        "nasal" => Some("nasal"),
        "ophthalmic" => Some("ophthalmic"),
        "inhalation" | "inhaled" => Some("inhalation"),
        _ => None,
    }
}

fn release_map(token: &str) -> Option<&'static str> {
    match token {
        "er" | "xr" | "extended" => Some("extended"),
        "dr" | "delayed" => Some("delayed"),
        "sr" | "sustained" => Some("sustained"),
        "ir" | "immediate" => Some("immediate"),
        _ => None,
    }
}

fn combination_count(original: &str, tokens: &[&str]) -> usize {
    let separators = original.matches('/').count() + original.matches('+').count();
    let conjunctions = tokens
        .iter()
        .filter(|token| matches!(**token, "and" | "with"))
        .count();
    1 + separators + conjunctions
}
