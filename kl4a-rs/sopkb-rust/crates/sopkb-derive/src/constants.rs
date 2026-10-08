//! Constants reproduced verbatim -- they are matched against and emitted in payloads.
//! docs/port/port-mapping-d-agent-mcp.md §3.1.

pub struct TaskDefinition {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub query_terms: &'static [&'static str],
}

pub const TASK_DEFINITIONS: &[TaskDefinition] = &[
    TaskDefinition {
        id: "eligibility-check",
        title: "Eligibility Check",
        description: "Use SOP requirements to decide what eligibility facts must be gathered or verified.",
        query_terms: &["eligibility", "identity", "contraindication", "clinical review"],
    },
    TaskDefinition {
        id: "prior-authorization",
        title: "Prior Authorization Packet",
        description: "Use SOP requirements to assemble payer, diagnosis, and documentation evidence.",
        query_terms: &["prior authorization", "payer", "diagnosis", "evidence", "packet"],
    },
    TaskDefinition {
        id: "follow-up-monitoring",
        title: "Follow-Up Monitoring",
        description: "Use SOP requirements to identify follow-up observations and review triggers.",
        query_terms: &["follow-up", "follow up", "dose", "tolerance", "adverse", "adherence"],
    },
];

/// NOT in `TASK_DEFINITIONS`: `task_definition("scenario-analysis")` raises, and
/// `agent_tasks` returns exactly 3 tasks, never this one.
pub const AUTO_TASK: TaskDefinition = TaskDefinition {
    id: "scenario-analysis",
    title: "Scenario Analysis",
    description: "Use scenario-detected concepts to retrieve related SOP knowledge, evidence, rules, and relations.",
    query_terms: &[],
};

pub const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "in", "is", "it", "no", "not", "of", "on",
    "or", "patient", "scenario", "the", "to", "with",
];

/// Linear scan of `TASK_DEFINITIONS`. Raises for `"scenario-analysis"` too (`AUTO_TASK`
/// is deliberately excluded from the lookup table).
pub fn task_definition(task_id: &str) -> Result<&'static TaskDefinition, String> {
    TASK_DEFINITIONS
        .iter()
        .find(|t| t.id == task_id)
        .ok_or_else(|| format!("unknown agent task: {task_id}"))
}
