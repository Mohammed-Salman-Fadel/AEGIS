use serde::Deserialize;

pub struct PlanParser;

impl PlanParser {
    pub fn new() -> Self { Self }

    pub fn parse(&self, raw: &str) -> ParsedPlan {
        let json = extract_json(raw);
        match serde_json::from_str::<PlannerResponse>(&json) {
            Ok(PlannerResponse::Final { answer, .. }) => ParsedPlan::Final { answer },
            Ok(PlannerResponse::Steps { steps, .. }) => ParsedPlan::Steps { steps },
            Err(_) => ParsedPlan::Final {
                answer: raw.trim().to_string(),
            },
        }
    }
}

#[derive(Debug)]
pub enum ParsedPlan {
    Final { answer: String },
    Steps { steps: Vec<PlanStep> },
}

#[derive(Debug, Deserialize)]
pub struct PlanStep {
    pub id:    String,
    pub tool:  String,
    pub input: String,
}

pub struct StepResult {
    pub step_id: String,
    pub output:  String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum PlannerResponse {
    Final {
        answer: String,
    },
    Steps {
        steps: Vec<PlanStep>,
    },
}

fn extract_json(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") {
        return trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string();
    }

    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        return trimmed[start..=end].to_string();
    }

    trimmed.to_string()
}
