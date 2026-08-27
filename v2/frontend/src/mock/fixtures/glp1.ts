/**
 * Mock bundle data adapted from
 * `sopkb-rust/fixtures/cases/reference/expected-python/bundle/` — the
 * "reference" golden-corpus case (4 markdown SOP sources, 12 sections, 16
 * knowledge items, all `proposed`). Field values (ids, checksums, text) are
 * copied verbatim from `.sopkb/inventory.json`, `.sopkb/sections.json` and
 * `.sopkb/items.json` in that fixture; `<TS>` placeholders are replaced with
 * a fixed mock timestamp.
 */
import type { KnowledgeItem, Section, Source } from '../../types/commands';

const TS = '2026-08-01T09:00:00Z';

export const BUNDLE_KEY = 'glp1-healthcare';
export const BUNDLE_TITLE = 'GLP-1 Healthcare Reference';

export const sources: Source[] = [
  {
    id: 'follow-up-monitoring-procedure-38ef67176baf',
    title: 'follow up monitoring procedure',
    type: 'markdown',
    original_path: 'sources/originals/follow-up-monitoring-procedure-38ef67176baf.md',
    normalized_path: 'sources/normalized/follow-up-monitoring-procedure-38ef67176baf.md',
    checksum: 'sha256:38ef67176baff96361fc84c36e57e10269e0bbfe1048d191866d13c612b31a9c',
    size: 427,
    modified_time: TS,
    parse_status: 'normalized',
    warnings: [],
    metadata: { source_filename: 'follow_up_monitoring_procedure.md' },
    extra: {},
  },
  {
    id: 'primary-care-glp1-sop-a3cca5c9520a',
    title: 'primary care glp1 sop',
    type: 'markdown',
    original_path: 'sources/originals/primary-care-glp1-sop-a3cca5c9520a.md',
    normalized_path: 'sources/normalized/primary-care-glp1-sop-a3cca5c9520a.md',
    checksum: 'sha256:a3cca5c9520a14fefb7daa776d51ec18363f0f76278b0182586887f72fafa5d5',
    size: 685,
    modified_time: TS,
    parse_status: 'normalized',
    warnings: [],
    metadata: { source_filename: 'primary_care_glp1_sop.md' },
    extra: {},
  },
  {
    id: 'prior-authorization-workflow-d2229b5da05c',
    title: 'prior authorization workflow',
    type: 'markdown',
    original_path: 'sources/originals/prior-authorization-workflow-d2229b5da05c.md',
    normalized_path: 'sources/normalized/prior-authorization-workflow-d2229b5da05c.md',
    checksum: 'sha256:d2229b5da05c2a08b2b5d3155179be119a5cb9994185a3a24c7687bc0b61a28c',
    size: 451,
    modified_time: TS,
    parse_status: 'normalized',
    warnings: [],
    metadata: { source_filename: 'prior_authorization_workflow.md' },
    extra: {},
  },
  {
    id: 'safety-policy-60f764ab3c8d',
    title: 'safety policy',
    type: 'markdown',
    original_path: 'sources/originals/safety-policy-60f764ab3c8d.md',
    normalized_path: 'sources/normalized/safety-policy-60f764ab3c8d.md',
    checksum: 'sha256:60f764ab3c8def954e6cd0e389c6195572f7ab024efa7a9ea755ac72ffbdf93c',
    size: 395,
    modified_time: TS,
    parse_status: 'normalized',
    warnings: [],
    metadata: { source_filename: 'safety_policy.md' },
    extra: {},
  },
];

export const sections: Section[] = [
  { id: 'section-follow-up-monitoring-procedure-38ef67176baf-001', source_id: 'follow-up-monitoring-procedure-38ef67176baf', heading: 'GLP-1 Follow-up Monitoring Procedure', semantic_role: 'procedure', start_pos: 0, end_pos: 40, normalized_path: 'sources/normalized/follow-up-monitoring-procedure-38ef67176baf.md', extra: {} },
  { id: 'section-follow-up-monitoring-procedure-38ef67176baf-002', source_id: 'follow-up-monitoring-procedure-38ef67176baf', heading: 'Follow-up Monitoring', semantic_role: 'section', start_pos: 40, end_pos: 234, normalized_path: 'sources/normalized/follow-up-monitoring-procedure-38ef67176baf.md', extra: {} },
  { id: 'section-follow-up-monitoring-procedure-38ef67176baf-003', source_id: 'follow-up-monitoring-procedure-38ef67176baf', heading: 'Dose Titration', semantic_role: 'section', start_pos: 234, end_pos: 412, normalized_path: 'sources/normalized/follow-up-monitoring-procedure-38ef67176baf.md', extra: {} },
  { id: 'section-primary-care-glp1-sop-a3cca5c9520a-001', source_id: 'primary-care-glp1-sop-a3cca5c9520a', heading: 'Primary Care GLP-1 Prescribing SOP', semantic_role: 'section', start_pos: 0, end_pos: 38, normalized_path: 'sources/normalized/primary-care-glp1-sop-a3cca5c9520a.md', extra: {} },
  { id: 'section-primary-care-glp1-sop-a3cca5c9520a-002', source_id: 'primary-care-glp1-sop-a3cca5c9520a', heading: 'Intake Requirements', semantic_role: 'policy', start_pos: 38, end_pos: 257, normalized_path: 'sources/normalized/primary-care-glp1-sop-a3cca5c9520a.md', extra: {} },
  { id: 'section-primary-care-glp1-sop-a3cca5c9520a-003', source_id: 'primary-care-glp1-sop-a3cca5c9520a', heading: 'Contraindication Screening', semantic_role: 'section', start_pos: 257, end_pos: 552, normalized_path: 'sources/normalized/primary-care-glp1-sop-a3cca5c9520a.md', extra: {} },
  { id: 'section-primary-care-glp1-sop-a3cca5c9520a-004', source_id: 'primary-care-glp1-sop-a3cca5c9520a', heading: 'Follow-up Monitoring', semantic_role: 'section', start_pos: 552, end_pos: 666, normalized_path: 'sources/normalized/primary-care-glp1-sop-a3cca5c9520a.md', extra: {} },
  { id: 'section-prior-authorization-workflow-d2229b5da05c-001', source_id: 'prior-authorization-workflow-d2229b5da05c', heading: 'GLP-1 Prior Authorization Workflow', semantic_role: 'procedure', start_pos: 0, end_pos: 38, normalized_path: 'sources/normalized/prior-authorization-workflow-d2229b5da05c.md', extra: {} },
  { id: 'section-prior-authorization-workflow-d2229b5da05c-002', source_id: 'prior-authorization-workflow-d2229b5da05c', heading: 'Prior Authorization Requirements', semantic_role: 'policy', start_pos: 38, end_pos: 266, normalized_path: 'sources/normalized/prior-authorization-workflow-d2229b5da05c.md', extra: {} },
  { id: 'section-prior-authorization-workflow-d2229b5da05c-003', source_id: 'prior-authorization-workflow-d2229b5da05c', heading: 'Denial Routing', semantic_role: 'section', start_pos: 266, end_pos: 436, normalized_path: 'sources/normalized/prior-authorization-workflow-d2229b5da05c.md', extra: {} },
  { id: 'section-safety-policy-60f764ab3c8d-001', source_id: 'safety-policy-60f764ab3c8d', heading: 'GLP-1 Safety Policy', semantic_role: 'policy', start_pos: 0, end_pos: 23, normalized_path: 'sources/normalized/safety-policy-60f764ab3c8d.md', extra: {} },
  { id: 'section-safety-policy-60f764ab3c8d-002', source_id: 'safety-policy-60f764ab3c8d', heading: 'Contraindication Screening', semantic_role: 'section', start_pos: 23, end_pos: 239, normalized_path: 'sources/normalized/safety-policy-60f764ab3c8d.md', extra: {} },
  { id: 'section-safety-policy-60f764ab3c8d-003', source_id: 'safety-policy-60f764ab3c8d', heading: 'Adverse Event Monitoring', semantic_role: 'section', start_pos: 239, end_pos: 382, normalized_path: 'sources/normalized/safety-policy-60f764ab3c8d.md', extra: {} },
];

function item(partial: Omit<KnowledgeItem, 'metadata' | 'extra' | 'derivation' | 'confidence' | 'span_status' | 'review_status'>): KnowledgeItem {
  return {
    ...partial,
    derivation: 'direct_text',
    confidence: 0.82,
    span_status: 'exact',
    review_status: 'proposed',
    metadata: { provider: 'fixture' },
    extra: {},
  };
}

export const items: KnowledgeItem[] = [
  item({ id: 'ki-follow-up-monitoring-procedure-38ef67176baf-000001', item_type: 'claim', subject: 'Follow-up Monitoring', predicate: 'should', object: 'Patients should receive follow-up contact within 14 days after GLP-1 therapy initiation.', source_id: 'follow-up-monitoring-procedure-38ef67176baf', section_id: 'section-follow-up-monitoring-procedure-38ef67176baf-002', source_text: 'Patients should receive follow-up contact within 14 days after GLP-1 therapy initiation.', start_pos: 65, end_pos: 153 }),
  item({ id: 'ki-follow-up-monitoring-procedure-38ef67176baf-000002', item_type: 'claim', subject: 'Follow-up Monitoring', predicate: 'records', object: 'Staff must record dose tolerance and adverse event symptoms during follow-up.', source_id: 'follow-up-monitoring-procedure-38ef67176baf', section_id: 'section-follow-up-monitoring-procedure-38ef67176baf-002', source_text: 'Staff must record dose tolerance and adverse event symptoms during follow-up.', start_pos: 155, end_pos: 232 }),
  item({ id: 'ki-follow-up-monitoring-procedure-38ef67176baf-000003', item_type: 'claim', subject: 'Dose Titration', predicate: 'requires', object: 'Clinicians must confirm patient tolerance before dose escalation.', source_id: 'follow-up-monitoring-procedure-38ef67176baf', section_id: 'section-follow-up-monitoring-procedure-38ef67176baf-003', source_text: 'Clinicians must confirm patient tolerance before dose escalation.', start_pos: 253, end_pos: 318 }),
  item({ id: 'ki-follow-up-monitoring-procedure-38ef67176baf-000004', item_type: 'claim', subject: 'Dose Titration', predicate: 'should', object: 'Clinicians should defer dose escalation when severe gastrointestinal symptoms are reported.', source_id: 'follow-up-monitoring-procedure-38ef67176baf', section_id: 'section-follow-up-monitoring-procedure-38ef67176baf-003', source_text: 'Clinicians should defer dose escalation when severe gastrointestinal symptoms are reported.', start_pos: 320, end_pos: 411 }),
  item({ id: 'ki-primary-care-glp1-sop-a3cca5c9520a-000005', item_type: 'claim', subject: 'Intake Requirements', predicate: 'requires', object: 'Clinicians must confirm patient identity before reviewing GLP-1 therapy eligibility.', source_id: 'primary-care-glp1-sop-a3cca5c9520a', section_id: 'section-primary-care-glp1-sop-a3cca5c9520a-002', source_text: 'Clinicians must confirm patient identity before reviewing GLP-1 therapy eligibility.', start_pos: 62, end_pos: 146 }),
  item({ id: 'ki-primary-care-glp1-sop-a3cca5c9520a-000006', item_type: 'claim', subject: 'Intake Requirements', predicate: 'requires', object: 'Clinicians must document current medications, diabetes history, weight history, and relevant comorbidities.', source_id: 'primary-care-glp1-sop-a3cca5c9520a', section_id: 'section-primary-care-glp1-sop-a3cca5c9520a-002', source_text: 'Clinicians must document current medications, diabetes history, weight history, and relevant comorbidities.', start_pos: 148, end_pos: 255 }),
  item({ id: 'ki-primary-care-glp1-sop-a3cca5c9520a-000007', item_type: 'claim', subject: 'Contraindication Screening', predicate: 'requires', object: 'Clinicians must screen for pregnancy, pancreatitis history, medullary thyroid carcinoma history, and multiple endocrine neoplasia type 2 history.', source_id: 'primary-care-glp1-sop-a3cca5c9520a', section_id: 'section-primary-care-glp1-sop-a3cca5c9520a-003', source_text: 'Clinicians must screen for pregnancy, pancreatitis history, medullary thyroid carcinoma history, and multiple endocrine neoplasia type 2 history.', start_pos: 288, end_pos: 433 }),
  item({ id: 'ki-primary-care-glp1-sop-a3cca5c9520a-000008', item_type: 'claim', subject: 'Contraindication Screening', predicate: 'should', object: 'Patients with uncertain contraindication history should be routed for clinical review before therapy is prescribed.', source_id: 'primary-care-glp1-sop-a3cca5c9520a', section_id: 'section-primary-care-glp1-sop-a3cca5c9520a-003', source_text: 'Patients with uncertain contraindication history should be routed for clinical review before therapy is prescribed.', start_pos: 435, end_pos: 550 }),
  item({ id: 'ki-primary-care-glp1-sop-a3cca5c9520a-000009', item_type: 'claim', subject: 'Follow-up Monitoring', predicate: 'should', object: 'Patients should receive follow-up contact within 30 days after GLP-1 therapy initiation.', source_id: 'primary-care-glp1-sop-a3cca5c9520a', section_id: 'section-primary-care-glp1-sop-a3cca5c9520a-004', source_text: 'Patients should receive follow-up contact within 30 days after GLP-1 therapy initiation.', start_pos: 577, end_pos: 665 }),
  item({ id: 'ki-prior-authorization-workflow-d2229b5da05c-000010', item_type: 'claim', subject: 'Prior Authorization Requirements', predicate: 'records', object: 'Staff must record payer requirements before submitting a GLP-1 prior authorization request.', source_id: 'prior-authorization-workflow-d2229b5da05c', section_id: 'section-prior-authorization-workflow-d2229b5da05c-002', source_text: 'Staff must record payer requirements before submitting a GLP-1 prior authorization request.', start_pos: 75, end_pos: 166 }),
  item({ id: 'ki-prior-authorization-workflow-d2229b5da05c-000011', item_type: 'claim', subject: 'Prior Authorization Requirements', predicate: 'requires', object: 'Staff must attach diagnosis evidence and prior therapy documentation when required by the payer.', source_id: 'prior-authorization-workflow-d2229b5da05c', section_id: 'section-prior-authorization-workflow-d2229b5da05c-002', source_text: 'Staff must attach diagnosis evidence and prior therapy documentation when required by the payer.', start_pos: 168, end_pos: 264 }),
  item({ id: 'ki-prior-authorization-workflow-d2229b5da05c-000012', item_type: 'claim', subject: 'Denial Routing', predicate: 'should', object: 'Staff should route prior authorization denials to the prescribing clinician for clinical review.', source_id: 'prior-authorization-workflow-d2229b5da05c', section_id: 'section-prior-authorization-workflow-d2229b5da05c-003', source_text: 'Staff should route prior authorization denials to the prescribing clinician for clinical review.', start_pos: 285, end_pos: 381 }),
  item({ id: 'ki-prior-authorization-workflow-d2229b5da05c-000013', item_type: 'claim', subject: 'Denial Routing', predicate: 'records', object: 'Staff must record appeal deadlines in the case note.', source_id: 'prior-authorization-workflow-d2229b5da05c', section_id: 'section-prior-authorization-workflow-d2229b5da05c-003', source_text: 'Staff must record appeal deadlines in the case note.', start_pos: 383, end_pos: 435 }),
  item({ id: 'ki-safety-policy-60f764ab3c8d-000014', item_type: 'claim', subject: 'Contraindication Screening', predicate: 'requires', object: 'Clinicians must confirm contraindication screening before prescribing GLP-1 therapy.', source_id: 'safety-policy-60f764ab3c8d', section_id: 'section-safety-policy-60f764ab3c8d-002', source_text: 'Clinicians must confirm contraindication screening before prescribing GLP-1 therapy.', start_pos: 54, end_pos: 138 }),
  item({ id: 'ki-safety-policy-60f764ab3c8d-000015', item_type: 'claim', subject: 'Contraindication Screening', predicate: 'requires', object: 'Patients with confirmed contraindications must not start GLP-1 therapy without specialist review.', source_id: 'safety-policy-60f764ab3c8d', section_id: 'section-safety-policy-60f764ab3c8d-002', source_text: 'Patients with confirmed contraindications must not start GLP-1 therapy without specialist review.', start_pos: 140, end_pos: 237 }),
  item({ id: 'ki-safety-policy-60f764ab3c8d-000016', item_type: 'claim', subject: 'Adverse Event Monitoring', predicate: 'should', object: 'Clinicians should record severe gastrointestinal symptoms, dehydration risk, and suspected pancreatitis symptoms.', source_id: 'safety-policy-60f764ab3c8d', section_id: 'section-safety-policy-60f764ab3c8d-003', source_text: 'Clinicians should record severe gastrointestinal symptoms, dehydration risk, and suspected pancreatitis symptoms.', start_pos: 268, end_pos: 381 }),
];

/** Conditions for the handful of items that read as genuinely conditional (mirrors the real `route-uncertain-case` rule). */
export const conditionsByItemId: Record<string, { fact: string; operator: string; label: string }> = {
  'ki-primary-care-glp1-sop-a3cca5c9520a-000008': {
    fact: 'case_uncertainty',
    operator: 'is_true',
    label: 'Case is uncertain',
  },
  'ki-follow-up-monitoring-procedure-38ef67176baf-000004': {
    fact: 'severe_gi_symptoms',
    operator: 'is_true',
    label: 'Severe gastrointestinal symptoms are reported',
  },
};

export const reportsMarkdown = {
  extraction_summary: `# Extraction Summary\n\n- Sources scanned: 4\n- Sections normalized: 12\n- Knowledge items mined: 16\n- Provider: fixture\n`,
  validation: `# Validation\n\nNo errors.\nNo warnings.\n`,
  conflicts: `# Conflicts\n\nNo conflicting knowledge items detected across sources for this bundle.\n`,
  freshness: `# Freshness\n\nAll 4 sources are up to date with their normalized text. No stale sources detected.\n`,
  review_summary: `# Review Summary\n\n- Proposed: 16\n- Approved: 0\n- Rejected: 0\n- Deferred: 0\n- Edited: 0\n`,
  export_summary: `# Export Summary\n\n- graph_json: exports/${BUNDLE_KEY}/graph/graph.json\n- rdf: exports/${BUNDLE_KEY}/graph/triples.ttl\n`,
  agent_guide: `# Agent Guide\n\nThis bundle exposes decision rules and knowledge relations for the GLP-1 primary-care\nprescribing workflow: intake, contraindication screening, dose titration, follow-up monitoring,\nprior authorization, and denial routing.\n`,
};

export const agentTasks = [
  { id: 'eligibility-check', title: 'Eligibility check', description: 'Determine whether a patient is eligible to start or continue GLP-1 therapy given intake and contraindication data.', matching_knowledge_count: 9 },
  { id: 'follow-up-monitoring', title: 'Follow-up monitoring', description: 'Guidance for scheduling and conducting follow-up contact after therapy initiation or dose changes.', matching_knowledge_count: 5 },
  { id: 'prior-authorization', title: 'Prior authorization', description: 'Steps for submitting and appealing a GLP-1 prior authorization request.', matching_knowledge_count: 4 },
];
