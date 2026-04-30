use crate::context::ConversationHistory;
use crate::plan_parser::StepResult;

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build_planning_prompt(
        &self,
        history: &ConversationHistory,
        user_message: &str,
    ) -> String {
        format!(
            r#"You are the AEGIS fallback workflow planner.

AEGIS is a local-only assistant. Decide whether the user can be answered directly or whether a short list of internal reasoning steps is needed.

Return valid JSON only. Do not wrap it in markdown.

If the user request can be answered directly, return exactly:
{{
  "type": "final",
  "answer": "the final answer for the user"
}}

If the request needs intermediate work, return exactly:
{{
  "type": "steps",
  "steps": [
    {{
      "id": "step_1",
      "tool": "think",
      "input": "one focused subtask"
    }}
  ]
}}

Allowed tools:
- think: ask the local model to reason about one focused subtask.

Keep plans short. Use at most 3 steps.

Conversation history:
{}

User message:
{}"#,
            format_history(history),
            user_message
        )
    }

    pub fn build_step_prompt(
        &self,
        history: &ConversationHistory,
        user_message: &str,
        step_input: &str,
    ) -> String {
        format!(
            r#"You are executing one internal AEGIS workflow step.

Conversation history:
{}

Original user message:
{}

Step to execute:
{}

Return the step result only. Be concise and concrete."#,
            format_history(history),
            user_message,
            step_input
        )
    }

    pub fn build_synthesis_prompt(
        &self,
        history: &ConversationHistory,
        user_message: &str,
        step_results: &[StepResult],
    ) -> String {
        let rendered_results = step_results
            .iter()
            .map(|result| format!("{}: {}", result.step_id, result.output))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"You are AEGIS, a private local-only AI assistant.

Use the conversation history, original user request, and executed workflow step results to produce the final answer.

Conversation history:
{}

Original user message:
{}

Step results:
{}

Final answer:"#,
            format_history(history),
            user_message,
            rendered_results
        )
    }
}

fn format_history(history: &ConversationHistory) -> String {
    if history.turns.is_empty() {
        return "<empty>".to_string();
    }

    history
        .turns
        .iter()
        .rev()
        .take(8)
        .rev()
        .map(|turn| format!("user: {}\nassistant: {}", turn.query, turn.response))
        .collect::<Vec<_>>()
        .join("\n\n")
}
