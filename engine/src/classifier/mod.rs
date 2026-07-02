use crate::workflow::WorkflowId;

/// Keyword-based classifier that maps a user query to a workflow.
///
/// Priority order (first match wins):
///   Summarize → CodeExplain/Debug/Generate → DocumentQA → Writing → Default
///
/// This is a fast, zero-latency pre-filter.  It can be replaced with a
/// small-model classifier later without changing anything outside this file.
pub struct Classifier;

impl Classifier {
    pub fn new() -> Self {
        Self
    }

    /// Classify a user message into a workflow.
    ///
    /// * `query` — the raw user message.
    /// * `has_attachments` — whether the request has imported documents attached.
    /// * `mode` — the chat mode hint from the frontend (general / coder / academic).
    pub fn classify(&self, query: &str, has_attachments: bool, mode: &str) -> WorkflowId {
        // Normalise once so helpers don't re-allocate.
        let lower = query.to_lowercase();
        let query = &lower;

        // ── 1. Summarisation ──────────────────────────────────────────
        // Triggered when documents are attached AND the user asks to
        // summarise them.
        if has_attachments
            && contains_any(
                query,
                &[
                    "summarize",
                    "summarise",
                    "summary",
                    "tl;dr",
                    "tldr",
                    "give me a summary",
                    "give me the gist",
                    "key points",
                    "main points",
                    "brief me",
                    "extract the main",
                    "what's this about",
                    "what is this about",
                    "overview of this",
                    "overview of the",
                ],
            )
        {
            return WorkflowId::Summarize;
        }

        // ── 2. Code-related workflows ─────────────────────────────────
        if is_code_scoped(query) {
            if contains_any(
                query,
                &[
                    "debug",
                    "bug",
                    "error",
                    "issue",
                    "not working",
                    "doesn't work",
                    "does not work",
                    "failing",
                    "crash",
                    "fix this",
                    "why is",
                    "broken",
                    "wrong output",
                    "incorrect",
                ],
            ) {
                return WorkflowId::CodeDebug;
            }
            if contains_any(
                query,
                &[
                    "write",
                    "create",
                    "implement",
                    "generate",
                    "build a",
                    "build an",
                    "new `class`",
                    "define a class",
                    "define a struct",
                    "new function",
                    "add a new",
                    "add feature",
                ],
            ) {
                return WorkflowId::CodeGenerate;
            }
            return WorkflowId::CodeExplain;
        }

        // ── 3. Document Q&A ───────────────────────────────────────────
        if has_attachments && is_document_scoped(query) {
            return WorkflowId::DocumentQA;
        }

        // ── 4. Writing / creative ─────────────────────────────────────
        if is_writing_scoped(query) {
            return WorkflowId::Writing;
        }

        // ── 5. Everything else ────────────────────────────────────────
        WorkflowId::Default
    }
}

// ── keyword matchers ──────────────────────────────────────────────────

/// Strong code terms — words that are strongly associated with
/// programming / software development when paired with an action verb.
const STRONG_CODE_TERMS: &[&str] = &[
    "code",
    "function",
    "method",
    "class",
    "variable",
    "module",
    "repo",
    "codebase",
    "rust",
    "python",
    "javascript",
    "typescript",
    "golang",
    "c++",
    "c#",
    "java",
    "html",
    "css",
    "sql",
    "api",
    "endpoint",
    "route",
    "middleware",
    "handler",
    "orchestrator",
    "compiler",
    "syntax",
    "refactor",
    "optimize",
    "algorithm",
    "data structure",
    "unit test",
    "test case",
    "integration test",
    "ci/cd",
    "pipeline",
    "deploy",
    "dependency",
    "import",
    "export",
    "async",
    "await",
    "promise",
    "callback",
    "exception",
    "try-catch",
    "try catch",
    "stack trace",
    "stacktrace",
    "null pointer",
    "segfault",
    "segmentation fault",
    "memory leak",
    "race condition",
];

/// Weak code terms — generic English words that CAN refer to programming
/// but also have non-technical meanings.  These only count towards the
/// "2+ code terms" threshold and cannot satisfy the "1 strong term + verb"
/// rule on their own.
const WEAK_CODE_TERMS: &[&str] = &[
    "implementation",
    "logic",
    "engine",
    "config",
    "configuration",
    "loop",
    "array",
    "string",
    "integer",
    "boolean",
    "null",
    "undefined",
    "filter",
    "reduce",
    "error handling",
    "test",
];

/// Action verbs that (together with a strong code term) indicate
/// a code-related request.  These are words that frequently appear
/// in programming questions specifically.
const CODE_ACTION_VERBS: &[&str] = &[
    "debug",
    "fix",
    "refactor",
    "optimize",
    "compile",
    "implement",
    "generate",
    "write",
    "create",
    "explain",
    "review",
    "analyze",
];

/// Returns true when `text` (pre-lowercased) contains at least one strong
/// code term together with a code action verb.  Also matches when 2+ code
/// terms (strong + weak combined) are present without a verb.
fn is_code_scoped(text: &str) -> bool {
    let has_strong = STRONG_CODE_TERMS.iter().any(|term| text.contains(term));
    let has_weak = WEAK_CODE_TERMS.iter().any(|term| text.contains(term));
    let has_action = CODE_ACTION_VERBS.iter().any(|verb| text.contains(verb));

    // 1 strong term + action verb → code
    if has_strong && has_action {
        return true;
    }

    // 2+ code terms (any mix of strong + weak) without verb → code
    let strong_count = STRONG_CODE_TERMS
        .iter()
        .filter(|term| text.contains(*term))
        .count();
    let weak_count = WEAK_CODE_TERMS
        .iter()
        .filter(|term| text.contains(*term))
        .count();
    let total_code_terms = strong_count + weak_count;

    total_code_terms >= 2
}

/// Returns true when `text` (pre-lowercased) refers to an attached document.
fn is_document_scoped(text: &str) -> bool {
    let document_references = [
        "the document",
        "this document",
        "that document",
        "attached document",
        "uploaded document",
        "imported document",
        "the pdf",
        "this pdf",
        "that pdf",
        "attached pdf",
        "uploaded pdf",
        "imported pdf",
        "the file",
        "this file",
        "that file",
        "attached file",
        "uploaded file",
        "imported file",
        "the paper",
        "this paper",
        "this article",
        "the article",
        "the chapter",
        "this chapter",
        "the report",
        "this report",
    ];

    let has_reference = document_references.iter().any(|phrase| text.contains(phrase));
    if !has_reference {
        return false;
    }

    let actions = [
        "summarize",
        "summarise",
        "summary",
        "explain",
        "analyze",
        "review",
        "read",
        "extract",
        "find",
        "from",
        "based on",
        "according to",
        "what does",
        "what is in",
        "tell me about",
        "looking for",
        "search",
        "question",
        "ask",
    ];

    actions.iter().any(|action| text.contains(action))
}

/// Returns true when `text` (pre-lowercased) asks for creative or prose
/// writing help.
fn is_writing_scoped(text: &str) -> bool {
    let writing_terms = [
        "write an essay",
        "write a story",
        "write a poem",
        "write a letter",
        "write an article",
        "write a blog",
        "write a post",
        "write a draft",
        "write a script",
        "write a scene",
        "write a screenplay",
        "write an email",
        "write a report",
        "write a review",
        "write a summary",
        "write a cover letter",
        "write my resume",
        "write my cv",
        "rewrite this",
        "write this",
        "write an",
        "write a",
        "creative writing",
        "short story",
        "narrative",
        "poem",
        "poetry",
        "essay",
        "article",
        "blog post",
        "newsletter",
        "email draft",
        "cover letter",
        "motivation letter",
        "speech",
        "lyrics",
        "song",
        "dialogue",
        "fiction",
        "novel",
        "chapter",
        "outline",
        "brainstorm",
        "rewrite",
        "proofread",
        "edit this",
        "improve this text",
        "make this sound",
        "tone",
        "voice",
        "style",
        "grammar",
        "spelling",
        "paraphrase",
        "compose",
        "draft a",
        "draft an",
    ];

    writing_terms.iter().any(|term| text.contains(term))
}

// ── helpers ───────────────────────────────────────────────────────────

/// Returns true when `text` (pre-lowercased) contains any of `patterns`.
fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

// ── public re-exports for the orchestrator ────────────────────────────

/// Check whether a pre-lowercased message refers to an attached document.
/// Exposed so the orchestrator can give a helpful early-return message
/// when the user asks about a document but none is attached.
pub(crate) fn message_refers_to_document(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("the document")
        || lower.contains("this document")
        || lower.contains("that document")
        || lower.contains("the pdf")
        || lower.contains("this pdf")
        || lower.contains("that pdf")
        || lower.contains("the file")
        || lower.contains("this file")
        || lower.contains("that file")
        || lower.contains("the paper")
        || lower.contains("this paper")
        || lower.contains("this article")
        || lower.contains("the article")
        || lower.contains("the chapter")
        || lower.contains("this chapter")
        || lower.contains("the report")
        || lower.contains("this report")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Default ───────────────────────────────────────────────────────

    #[test]
    fn general_greeting_is_default() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Hello, how are you?", false, "general"),
            WorkflowId::Default
        );
    }

    #[test]
    fn weather_question_is_default() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("What's the weather like today?", false, "general"),
            WorkflowId::Default
        );
    }

    #[test]
    fn generic_file_mention_is_default() {
        let c = Classifier::new();
        // "file" + "where" is in the weak list — 1 weak term alone should
        // NOT trigger code mode (needs 2+ code terms total).
        assert_eq!(
            c.classify("Where is the file I downloaded?", false, "general"),
            WorkflowId::Default
        );
    }

    #[test]
    fn generic_map_question_is_default() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Can you show me a map?", false, "general"),
            WorkflowId::Default
        );
    }

    // ── CodeExplain ───────────────────────────────────────────────────

    #[test]
    fn explain_code_triggers_code_explain() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("How does the Rust borrow checker work?", false, "general"),
            WorkflowId::CodeExplain
        );
    }

    #[test]
    fn explain_function_triggers_code_explain() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Explain this Python function", false, "general"),
            WorkflowId::CodeExplain
        );
    }

    #[test]
    fn query_about_codebase_triggers_code_explain() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Show me the code for the orchestrator", false, "general"),
            WorkflowId::CodeExplain
        );
    }

    #[test]
    fn single_code_term_without_action_does_not_trigger() {
        let c = Classifier::new();
        // 1 strong term ("python") + 1 weak term ("engine") = 2 code
        // terms, which would trigger.  So test with just 1 strong term
        // and 0 action verbs:
        assert_eq!(
            c.classify("I like Python", false, "general"),
            WorkflowId::Default
        );
    }

    #[test]
    fn two_code_terms_without_verb_trigger() {
        let c = Classifier::new();
        // 2 strong terms = enough even without action verb
        assert_eq!(
            c.classify("Python async code", false, "general"),
            WorkflowId::CodeExplain
        );
    }

    // ── CodeDebug ─────────────────────────────────────────────────────

    #[test]
    fn debug_request_triggers_code_debug() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Debug this Rust function, it's not working", false, "general"),
            WorkflowId::CodeDebug
        );
    }

    #[test]
    fn bug_report_triggers_code_debug() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Why is my Python code throwing an error?", false, "general"),
            WorkflowId::CodeDebug
        );
    }

    // ── CodeGenerate ──────────────────────────────────────────────────

    #[test]
    fn code_generation_triggers_code_generate() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Write a Rust function that sorts a list", false, "general"),
            WorkflowId::CodeGenerate
        );
    }

    #[test]
    fn implement_class_triggers_code_generate() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Create a new function for handling API requests", false, "general"),
            WorkflowId::CodeGenerate
        );
    }

    // ── Summarize ─────────────────────────────────────────────────────

    #[test]
    fn summarize_with_attachment_triggers_summarize() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Summarize this document", true, "general"),
            WorkflowId::Summarize
        );
    }

    // ── DocumentQA ────────────────────────────────────────────────────

    #[test]
    fn document_qa_with_attachment_triggers_document_qa() {
        let c = Classifier::new();
        assert_eq!(
            c.classify(
                "What does this document say about machine learning?",
                true,
                "general"
            ),
            WorkflowId::DocumentQA
        );
    }

    // ── Writing ───────────────────────────────────────────────────────

    #[test]
    fn essay_request_triggers_writing() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Write an essay about climate change", false, "general"),
            WorkflowId::Writing
        );
    }

    #[test]
    fn creative_writing_triggers_writing() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Write a short story about a robot", false, "general"),
            WorkflowId::Writing
        );
    }

    #[test]
    fn proofread_triggers_writing() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Proofread this paragraph for grammar", false, "general"),
            WorkflowId::Writing
        );
    }

    #[test]
    fn email_writing_triggers_writing() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Write an email to my boss", false, "general"),
            WorkflowId::Writing
        );
    }

    #[test]
    fn compose_triggers_writing() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Compose a poem about autumn", false, "general"),
            WorkflowId::Writing
        );
    }

    // ── Priority ordering ─────────────────────────────────────────────

    #[test]
    fn summarize_takes_priority_over_code() {
        let c = Classifier::new();
        assert_eq!(
            c.classify("Summarize the code in this document", true, "general"),
            WorkflowId::Summarize
        );
    }

    // ── message_refers_to_document ────────────────────────────────────

    #[test]
    fn refers_to_document_phrases() {
        assert!(message_refers_to_document("Summarize the document"));
        assert!(message_refers_to_document("What's in this PDF?"));
        assert!(message_refers_to_document("Explain that file"));
        assert!(message_refers_to_document("Read the paper"));
        assert!(message_refers_to_document("Analyze this article"));
        assert!(message_refers_to_document("Summarize the chapter"));
        assert!(message_refers_to_document("Find this report"));
        assert!(!message_refers_to_document("What is the weather"));
        assert!(!message_refers_to_document("Write a poem"));
    }
}
