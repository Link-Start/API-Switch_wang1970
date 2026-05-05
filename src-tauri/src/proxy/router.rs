use super::circuit_breaker::CircuitBreaker;
use crate::database::ApiEntry;
use chrono::NaiveDate;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Parse response_ms field to milliseconds.
/// Supports raw milliseconds ("1234") and legacy display values ("1.2s" / "350ms").
/// Returns None if missing or unparseable.
fn parse_response_ms(entry: &ApiEntry) -> Option<i64> {
    let value = entry.response_ms.as_deref()?.trim().to_ascii_lowercase();
    if value.is_empty() || value == "x" {
        return None;
    }
    if let Some(milliseconds) = value.strip_suffix("ms") {
        return milliseconds.parse::<f64>().ok().map(|ms| ms.round() as i64);
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds.parse::<f64>().ok().map(|s| (s * 1000.0).round() as i64);
    }
    value.parse::<f64>().ok().map(|ms| ms.round() as i64)
}

/// Sort entries by response time ascending; entries without measurement go last.
fn sort_by_latency(entries: &mut [ApiEntry]) {
    entries.sort_by_key(|e| parse_response_ms(e).unwrap_or(i64::MAX));
}

/// Sort entries by sort_index ascending (user's custom order).
fn sort_by_index(entries: &mut [ApiEntry]) {
    entries.sort_by_key(|e| e.sort_index);
}

fn parse_release_date(entry: &ApiEntry) -> Option<NaiveDate> {
    let value = entry.release_date.as_deref()?.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Some(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d") {
        return Some(date);
    }
    NaiveDate::parse_from_str(value, "%Y%m%d").ok()
}

/// Sort entries by release date descending; entries without release date go last.
fn sort_by_release_date(entries: &mut [ApiEntry]) {
    entries.sort_by(|a, b| {
        let date_cmp = parse_release_date(b).cmp(&parse_release_date(a));
        if date_cmp == std::cmp::Ordering::Equal {
            a.sort_index.cmp(&b.sort_index)
        } else {
            date_cmp
        }
    });
}

fn is_not_cooled_down(entry: &ApiEntry) -> bool {
    entry
        .cooldown_until
        .map(|until| until <= chrono::Utc::now().timestamp())
        .unwrap_or(true)
}

/// Resolve which entries to try for a given model request.
/// Returns an ordered list of entries to attempt (failover in order).
///
/// Rules (priority order):
    /// 1. `auto`: prefer current active_group within the enabled + non-cooldown pool, sorted by `sort_mode`; fallback to full auto pool.
/// 2. Group exact match: `group_name == model` → group entries, sorted by `sort_mode`.
/// 3. Model substring match: request length ≥ 5 → `model.contains(request)`, sorted by `sort_mode`.
/// 4. Fallback: auto_entries (enabled + non-cooldown pool).
pub async fn resolve(
    model: &str,
    all_entries: &[ApiEntry],
    auto_entries: &[ApiEntry],
    circuit_breakers: &RwLock<HashMap<String, CircuitBreaker>>,
    _sort_mode: &str,
    active_group: &str,
) -> Vec<ApiEntry> {

    let breakers = circuit_breakers.read().await;

    // Helper: filter out circuit-open entries, then sort by sort_mode
    // Filter out circuit-open entries and sort by user's custom sort_index, ignoring sort_mode.
    let filter_available = |entries: &[ApiEntry]| -> Vec<ApiEntry> {
        let mut available: Vec<ApiEntry> = entries
            .iter()
            .filter(|e| {
                if let Some(cb) = breakers.get(&e.id) {
                    cb.is_available()
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        // Previously applied sort_mode here, but now we always sort by sort_index to ensure uniform ordering.
        sort_by_index(&mut available);
        available
    };

    // 1. AUTO mode: use active group first, then fallback to full auto pool
    // Updated to prioritize DB sort_index ordering, ignoring default_sort_mode.
    if model.is_empty() || model.eq_ignore_ascii_case("auto") {
        // Filter out circuit-open entries for active group
        let mut active_group_entries: Vec<ApiEntry> = auto_entries
            .iter()
            .filter(|e| e.group_name.as_deref().unwrap_or("auto") == active_group)
            .filter(|e| {
                if let Some(cb) = breakers.get(&e.id) {
                    cb.is_available()
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        // Sort by sort_index (custom order)
        sort_by_index(&mut active_group_entries);
        if !active_group_entries.is_empty() {
            return active_group_entries;
        }

        // Fallback: all auto entries, filtered and sorted by sort_index
        let mut fallback_auto: Vec<ApiEntry> = auto_entries
            .iter()
            .filter(|e| {
                if let Some(cb) = breakers.get(&e.id) {
                    cb.is_available()
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        sort_by_index(&mut fallback_auto);
        return fallback_auto;
    }

    // 2. Group exact match: `group_name == model`
    let all_available = filter_available(all_entries);
    let group_matches: Vec<ApiEntry> = all_available
        .iter()
        .filter(|e| {
            e.enabled
                && e.group_name.as_deref() == Some(model)
                && is_not_cooled_down(e)
        })
        .cloned()
        .collect();

    if !group_matches.is_empty() {
        return group_matches;
    }

    // 3. Model substring match: request length ≥ 5
    if model.len() >= 5 {
        let model_matches: Vec<ApiEntry> = all_available
            .iter()
            .filter(|e| {
                e.enabled && e.model.contains(model) && is_not_cooled_down(e)
            })
            .cloned()
            .collect();

    if !model_matches.is_empty() {
        return model_matches;
    }
    }

    // 4. Fallback to AUTO
    filter_available(auto_entries)
}

/// Apply sort mode to entries: "custom" → sort_index, "fastest" → latency, "latest" → release_date.
pub(crate) fn apply_sort_mode(entries: &mut [ApiEntry], sort_mode: &str) {
    match sort_mode {
        "fastest" => sort_by_latency(entries),
        "latest" => sort_by_release_date(entries),
        _ => sort_by_index(entries),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, model: &str, enabled: bool, sort_index: i32) -> ApiEntry {
        ApiEntry {
            id: id.to_string(),
            channel_id: format!("channel-{id}"),
            model: model.to_string(),
            display_name: model.to_string(),
            sort_index,
            enabled,
            cooldown_until: None,
            circuit_state: "closed".to_string(),
            created_at: 0,
            updated_at: 0,
            channel_name: Some(format!("channel-{id}")),
            channel_api_type: Some("openai".to_string()),
            response_ms: None,
            owned_by: None,
            provider_logo: None,
            release_date: None,
            model_meta_zh: None,
            model_meta_en: None,
            group_name: None,
        }
    }

    #[tokio::test]
    async fn auto_prefers_active_group_before_full_auto_fallback() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![
            entry_with_group("auto-first", "gpt-4o", true, 0, "auto"),
            entry_with_group("coding-first", "claude-3", true, 1, "coding"),
            entry_with_group("coding-second", "gemini-pro", true, 2, "coding"),
        ];

        let resolved = resolve("auto", &enabled, &enabled, &breakers, "custom", "coding").await;

        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["coding-first", "coding-second"]);
    }

    #[tokio::test]
    async fn auto_uses_enabled_entries_in_order() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![
            entry("first", "gpt-4o", true, 0),
            entry("second", "claude-3", true, 1),
            entry("third", "gemini-pro", true, 2),
        ];

        let resolved = resolve("auto", &enabled, &enabled, &breakers, "custom", "auto").await;

        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["first", "second", "third"]);
    }

    #[tokio::test]
    async fn group_exact_match_routes_to_group_entries() {
        let breakers = RwLock::new(HashMap::new());
        let all = vec![
            entry_with_group("match1", "gpt-4o", true, 0, "coding"),
            entry_with_group("match2", "claude-3", true, 1, "coding"),
            entry_with_group("other", "gemini-pro", true, 2, "other"),
        ];

        let resolved = resolve("coding", &all, &all, &breakers, "custom", "auto").await;

        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["match1", "match2"]);
    }

    #[tokio::test]
    async fn group_match_without_enabled_entries_falls_back_to_auto() {
        let breakers = RwLock::new(HashMap::new());
        let all = vec![
            entry_with_group("disabled-match", "gpt-4o", false, 0, "coding"),
            entry_with_group("fallback", "claude-3", true, 1, "other"),
        ];
        let auto = vec![entry_with_group("fallback", "claude-3", true, 1, "other")];

        let resolved = resolve("coding", &all, &auto, &breakers, "custom", "auto").await;

        // No enabled entries in "coding" group, falls back to auto
        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["fallback"]);
    }

    #[tokio::test]
    async fn model_substring_match_routes_to_matching_entries() {
        let breakers = RwLock::new(HashMap::new());
        let all = vec![
            entry_with_group("match1", "[aa]gpt-4o-aabb", true, 0, "auto"),
            entry_with_group("match2", "[bb]gpt-4o-ccdd", true, 1, "auto"),
            entry_with_group("other", "claude-3", true, 2, "auto"),
        ];

        let resolved = resolve("gpt-4o", &all, &all, &breakers, "custom", "auto").await;

        // "gpt-4o" is 5 chars, matches substring in model names
        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["match1", "match2"]);
    }

    #[tokio::test]
    async fn model_substring_shorter_than_5_falls_back_to_auto() {
        let breakers = RwLock::new(HashMap::new());
        let all = vec![
            entry_with_group("match", "gpt-4o", true, 0, "auto"),
            entry_with_group("fallback", "claude-3", true, 1, "auto"),
        ];

        // "4o" is only 2 chars, substring match is skipped
        let resolved = resolve("4o", &all, &all, &breakers, "custom", "auto").await;

        // Falls back to auto (no group match, no substring match)
        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["match", "fallback"]);
    }

    #[tokio::test]
    async fn group_match_takes_priority_over_substring_match() {
        let breakers = RwLock::new(HashMap::new());
        let all = vec![
            entry_with_group("group-match", "coding-model", true, 0, "coding"),
            entry_with_group("substring-match", "coding-ai", true, 1, "auto"),
        ];

        let resolved = resolve("coding", &all, &all, &breakers, "custom", "auto").await;

        // Group match takes priority, only returns group entries
        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["group-match"]);
    }

    #[tokio::test]
    async fn exact_model_can_route_disabled_entry_but_auto_skips_it() {
        let breakers = RwLock::new(HashMap::new());
        let disabled = entry_with_group("disabled-match", "target-model", false, 0, "target-model");
        let fallback = entry("fallback", "fallback-model", true, 1);
        let all = vec![disabled, fallback.clone()];
        let auto = vec![fallback];

        let auto_resolved = resolve("auto", &all, &auto, &breakers, "custom", "auto").await;
        assert_eq!(auto_resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["fallback"]);

        // With new logic, disabled entries are never returned (only enabled entries)
        let exact_resolved = resolve("target-model", &all, &auto, &breakers, "custom", "auto").await;
        assert_eq!(exact_resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["fallback"]);
    }

    #[tokio::test]
    async fn exact_model_custom_keeps_match_before_auto_fallback() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![
            entry_with_group("fallback-first", "fallback-model", true, 0, "fallback-model"),
            entry_with_group("match", "target-model", true, 2, "target-model"),
            entry_with_group("fallback-second", "other-model", true, 1, "other-model"),
        ];

        let resolved = resolve("target-model", &enabled, &enabled, &breakers, "custom", "auto").await;

        // With new logic, group exact match returns only group entries (no auto fallback appended)
        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["match"]);
    }

    fn entry_with_group(id: &str, model: &str, enabled: bool, sort_index: i32, group: &str) -> ApiEntry {
        let mut e = entry(id, model, enabled, sort_index);
        e.group_name = Some(group.to_string());
        e
    }

    #[tokio::test]
    async fn exact_model_without_enabled_match_falls_back_to_auto_pool() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![entry("fallback", "claude-3", true, 1)];

        let resolved = resolve("gpt-4o", &enabled, &enabled, &breakers, "custom", "auto").await;

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, "fallback");
    }

    #[tokio::test]
    async fn circuit_open_entries_are_skipped() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![
            entry("open", "gpt-4o", true, 0),
            entry("fallback", "claude-3", true, 1),
        ];
        {
            let mut guard = breakers.write().await;
            let cb = CircuitBreaker::new(60);
            cb.record_failure(1);
            guard.insert("open".to_string(), cb);
        }

        let resolved = resolve("gpt-4o", &enabled, &enabled, &breakers, "custom", "auto").await;

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, "fallback");
    }

    #[tokio::test]
    async fn latest_sort_uses_release_date_descending() {
        let breakers = RwLock::new(HashMap::new());
        let mut older = entry("older", "old-model", true, 0);
        older.release_date = Some("2023-01-15".to_string());
        let mut newer = entry("newer", "new-model", true, 1);
        newer.release_date = Some("2024-08".to_string());
        let missing = entry("missing", "unknown-model", true, 2);
        let mut newest = entry("newest", "newest-model", true, 3);
        newest.release_date = Some("20240902".to_string());
        let enabled = vec![older, missing, newest, newer];

        let resolved = resolve("auto", &enabled, &enabled, &breakers, "latest", "auto").await;

        // Expect order follows DB sort_index ordering (custom order)
        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["older", "newer", "missing", "newest"]);
    }

    #[tokio::test]
    async fn fastest_sort_uses_response_ms_ascending_with_legacy_units() {
        let breakers = RwLock::new(HashMap::new());
        let mut slow = entry("slow", "slow-model", true, 0);
        slow.response_ms = Some("1.2s".to_string());
        let mut fast = entry("fast", "fast-model", true, 1);
        fast.response_ms = Some("350ms".to_string());
        let missing = entry("missing", "unknown-model", true, 2);
        let enabled = vec![slow, missing, fast];

        let resolved = resolve("auto", &enabled, &enabled, &breakers, "fastest", "auto").await;

        // Expect order follows DB sort_index ordering (custom order)
        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["slow", "fast", "missing"]);
    }

    #[tokio::test]
    async fn custom_auto_uses_sort_index_order() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![
            entry("third", "third-model", true, 2),
            entry("first", "first-model", true, 0),
            entry("second", "second-model", true, 1),
        ];

        let resolved = resolve("auto", &enabled, &enabled, &breakers, "custom", "auto").await;

        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["first", "second", "third"]);
    }

    #[tokio::test]
    async fn exact_model_skips_cooled_down_match_and_falls_back_to_auto() {
        let breakers = RwLock::new(HashMap::new());
        let mut cooled_down = entry("cooled-down-match", "target-model", false, 0);
        cooled_down.cooldown_until = Some(chrono::Utc::now().timestamp() + 60);
        let fallback = entry("fallback", "fallback-model", true, 1);
        let all = vec![cooled_down, fallback.clone()];
        let auto = vec![fallback];

        let resolved = resolve("target-model", &all, &auto, &breakers, "custom", "auto").await;

        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["fallback"]);
    }
}
