use crate::context::RequestContext;
use serde::Serialize;

/// Fraction of the model's context window reserved for conversation history.
/// The remaining context is used for the system prompt, RAG/code/project/zotero
/// context sections, the user query, and the model's output.
const HISTORY_BUDGET_FRACTION: f64 = 0.30;

/// Absolute cap on history token budget — prevents allocating an excessive
/// amount even on very large context windows (e.g. 128K+ models).
const MAX_HISTORY_BUDGET: usize = 16_384;

/// Minimum history budget — ensures at least some conversation context even
/// when the model context window is very small or unknown.
const MIN_HISTORY_BUDGET: usize = 1_024;

/// Marker inserted into the first remaining turn when older turns have been
/// dropped, so the model knows conversation context was trimmed.
const COMPACTION_PREFIX: &str = "[Earlier conversation turns were compacted to save context. {count} earlier messages removed.]";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CompactionReport {
    pub removed_turns: usize,
    pub kept_turns: usize,
    pub estimated_tokens_before: usize,
    pub estimated_tokens_after: usize,
    pub history_budget_tokens: usize,
    pub context_window_tokens: usize,
}

pub struct Compactor;

impl Compactor {
    pub fn new() -> Self {
        Self
    }

    /// Compute the token budget allocated to conversation history.
    fn history_budget(context_window: usize) -> usize {
        let from_fraction = (context_window as f64 * HISTORY_BUDGET_FRACTION) as usize;
        let budget = from_fraction.min(MAX_HISTORY_BUDGET);
        budget.max(MIN_HISTORY_BUDGET)
    }

    /// Compact `ctx.history.turns` in place: drop the oldest turns until the
    /// remaining history fits within the token budget derived from
    /// `context_window`.
    ///
    /// When turns are dropped, a short note is injected into the first
    /// remaining user query so the model is aware of the truncation.
    pub fn compact(
        &self,
        ctx: &mut RequestContext,
        context_window: usize,
    ) -> Option<CompactionReport> {
        let budget = Self::history_budget(context_window);

        if ctx.history.turns.is_empty() {
            return None;
        }

        let estimated_tokens_before = ctx
            .history
            .turns
            .iter()
            .map(|turn| turn.token_estimate())
            .sum();

        // Walk turns from newest to oldest, summing their token cost.
        // We keep the most recent turns that fit within the budget.
        let mut running_total: usize = 0;
        let mut keep_count: usize = 0;

        for turn in ctx.history.turns.iter().rev() {
            let cost = turn.token_estimate();
            if running_total + cost > budget && keep_count > 0 {
                // Stop adding turns once we'd overshoot AND we already have
                // at least one turn selected. Ensures we never end up with
                // zero turns — the most recent turn is always retained.
                break;
            }
            running_total += cost;
            keep_count += 1;
        }

        // If `keep_count` covers all turns, nothing to drop.
        if keep_count >= ctx.history.turns.len() {
            return None;
        }

        let drop_count = ctx.history.turns.len() - keep_count;

        // Remove the oldest turns (indices 0 .. drop_count)
        let _dropped: Vec<_> = ctx.history.turns.drain(..drop_count).collect();

        // Inject a compaction notice into the first remaining user query so
        // the model is aware history was trimmed.
        if let Some(first_remaining) = ctx.history.turns.first_mut() {
            let note = COMPACTION_PREFIX.replace("{count}", &drop_count.to_string());
            first_remaining.query = format!("{}\n\n{}", note, first_remaining.query);
        }

        let estimated_tokens_after = ctx
            .history
            .turns
            .iter()
            .map(|turn| turn.token_estimate())
            .sum();

        Some(CompactionReport {
            removed_turns: drop_count,
            kept_turns: ctx.history.turns.len(),
            estimated_tokens_before,
            estimated_tokens_after,
            history_budget_tokens: budget,
            context_window_tokens: context_window,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ConversationHistory, Turn};
    use chrono::Utc;

    fn make_turn(
        query: &str,
        response: &str,
        prompt_tokens: Option<usize>,
        completion_tokens: Option<usize>,
    ) -> Turn {
        Turn {
            query: query.to_string(),
            response: response.to_string(),
            created_at: Utc::now(),
            edited: false,
            prompt_tokens,
            completion_tokens,
        }
    }

    fn make_ctx(turns: Vec<Turn>) -> RequestContext {
        RequestContext::new(
            "test-session".to_string(),
            "test query".to_string(),
            ConversationHistory { turns },
            crate::model_registry::ModelProfile {
                name: "test-model".to_string(),
                context_window: 8192,
                output_reserve: 512,
            },
        )
    }

    #[test]
    fn empty_history_is_noop() {
        let compactor = Compactor::new();
        let mut ctx = make_ctx(vec![]);
        let report = compactor.compact(&mut ctx, 8192);
        assert!(ctx.history.turns.is_empty());
        assert!(report.is_none());
    }

    #[test]
    fn single_turn_stays_within_budget() {
        let compactor = Compactor::new();
        let turn = make_turn("hello", "hi there", Some(10), Some(5));
        let mut ctx = make_ctx(vec![turn]);
        let report = compactor.compact(&mut ctx, 8192);
        assert_eq!(ctx.history.turns.len(), 1);
        assert!(report.is_none());
        // No compaction notice for a single turn
        assert!(!ctx.history.turns[0].query.starts_with('['));
    }

    #[test]
    fn drops_old_turns_when_over_budget() {
        let compactor = Compactor::new();
        let turns = vec![
            make_turn("old message", "old response", Some(500), Some(500)),
            make_turn("middle message", "middle response", Some(500), Some(500)),
            make_turn("recent message", "recent response", Some(10), Some(5)),
        ];
        let mut ctx = make_ctx(turns);
        let report = compactor
            .compact(&mut ctx, 2000)
            .expect("expected compaction report"); // tiny context window -> tiny budget

        // Should keep at least 1 turn
        assert!(!ctx.history.turns.is_empty());
        assert!(ctx.history.turns.len() < 3);
        assert_eq!(report.removed_turns, 1);
        assert_eq!(report.kept_turns, 2);
        assert!(report.estimated_tokens_before > report.estimated_tokens_after);
        // The remaining turn(s) should have the compaction notice
        assert!(ctx.history.turns[0].query.starts_with('['));
    }

    #[test]
    fn budget_respects_window_sizes() {
        // Huge context window = large budget
        let large = Compactor::history_budget(128_000);
        assert_eq!(large, MAX_HISTORY_BUDGET); // capped

        // Tiny context window = minimum budget
        let tiny = Compactor::history_budget(512);
        assert_eq!(tiny, MIN_HISTORY_BUDGET);

        // Normal 8K model
        let normal = Compactor::history_budget(8192);
        assert_eq!(normal, 2457); // 8192 * 0.3 = 2457
    }

    #[test]
    fn never_drops_all_turns() {
        let compactor = Compactor::new();
        let huge_turn = make_turn(
            &"a".repeat(100_000),
            &"b".repeat(100_000),
            Some(50_000),
            Some(50_000),
        );
        let mut ctx = make_ctx(vec![huge_turn]);
        let report = compactor.compact(&mut ctx, 512); // tiny budget, huge single turn
        // The single turn is ALWAYS retained (keep_count starts at 0,
        // so the condition `keep_count > 0` prevents the break from
        // triggering before the first turn is accepted)
        assert_eq!(ctx.history.turns.len(), 1);
        assert!(report.is_none());
    }
}
