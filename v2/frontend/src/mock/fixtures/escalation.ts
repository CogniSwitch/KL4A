/**
 * Mock bundle data adapted from
 * `sopkb-rust/fixtures/cases/reviewed/expected-python/bundle/` — the
 * "reviewed" golden-corpus case. Deliberately smaller (1 source, 6 items)
 * and already carries review history (`.sopkb/reviews.json`, copied
 * verbatim below) so the app has a second, differently-shaped bundle to
 * exercise the bundle picker, and a bundle that already has
 * approved/rejected/deferred/edited items to exercise `allowed_actions`
 * gating without the user having to review anything first.
 */
import type { KnowledgeItem, ReviewEvent, Section, Source } from '../../types/commands';

const TS = '2026-07-20T14:30:00Z';

export const BUNDLE_KEY = 'escalation-runbook';
export const BUNDLE_TITLE = 'Reviewed Bundle Case';

export const sources: Source[] = [
  {
    id: 'escalation-runbook-096d9a2e502e',
    title: 'escalation runbook',
    type: 'markdown',
    original_path: 'sources/originals/escalation-runbook-096d9a2e502e.md',
    normalized_path: 'sources/normalized/escalation-runbook-096d9a2e502e.md',
    checksum: 'sha256:096d9a2e502e9a9e51c9a1fb22c434299623828263d448a22d03948ac35c6f83',
    size: 603,
    modified_time: TS,
    parse_status: 'normalized',
    warnings: [],
    metadata: { source_filename: 'escalation-runbook.md' },
    extra: {},
  },
];

export const sections: Section[] = [
  { id: 'section-escalation-runbook-096d9a2e502e-001', source_id: 'escalation-runbook-096d9a2e502e', heading: 'Escalation Runbook', semantic_role: 'section', start_pos: 0, end_pos: 22, normalized_path: 'sources/normalized/escalation-runbook-096d9a2e502e.md', extra: {} },
  { id: 'section-escalation-runbook-096d9a2e502e-002', source_id: 'escalation-runbook-096d9a2e502e', heading: 'Initial Notification', semantic_role: 'section', start_pos: 22, end_pos: 115, normalized_path: 'sources/normalized/escalation-runbook-096d9a2e502e.md', extra: {} },
  { id: 'section-escalation-runbook-096d9a2e502e-003', source_id: 'escalation-runbook-096d9a2e502e', heading: 'Secondary Contact', semantic_role: 'section', start_pos: 115, end_pos: 216, normalized_path: 'sources/normalized/escalation-runbook-096d9a2e502e.md', extra: {} },
  { id: 'section-escalation-runbook-096d9a2e502e-004', source_id: 'escalation-runbook-096d9a2e502e', heading: 'Vendor Escalation', semantic_role: 'section', start_pos: 216, end_pos: 308, normalized_path: 'sources/normalized/escalation-runbook-096d9a2e502e.md', extra: {} },
  { id: 'section-escalation-runbook-096d9a2e502e-005', source_id: 'escalation-runbook-096d9a2e502e', heading: 'Legal Notification', semantic_role: 'section', start_pos: 308, end_pos: 403, normalized_path: 'sources/normalized/escalation-runbook-096d9a2e502e.md', extra: {} },
  { id: 'section-escalation-runbook-096d9a2e502e-006', source_id: 'escalation-runbook-096d9a2e502e', heading: 'Patient Communication', semantic_role: 'section', start_pos: 403, end_pos: 507, normalized_path: 'sources/normalized/escalation-runbook-096d9a2e502e.md', extra: {} },
  { id: 'section-escalation-runbook-096d9a2e502e-007', source_id: 'escalation-runbook-096d9a2e502e', heading: 'Post-Incident Review', semantic_role: 'section', start_pos: 507, end_pos: 603, normalized_path: 'sources/normalized/escalation-runbook-096d9a2e502e.md', extra: {} },
];

function item(
  partial: Omit<KnowledgeItem, 'metadata' | 'extra' | 'derivation' | 'confidence' | 'span_status' | 'review_status'>,
  review_status: KnowledgeItem['review_status'] = 'proposed',
): KnowledgeItem {
  return {
    ...partial,
    derivation: 'direct_text',
    confidence: 0.82,
    span_status: 'exact',
    review_status,
    metadata: { provider: 'fixture' },
    extra: {},
  };
}

// Review outcomes below mirror `.sopkb/reviews.json` in the fixture: item 1
// approved, item 2 rejected, item 3 deferred, item 4 approved-then-commented
// (comment does not change status), item 5 edited (subject/predicate/object/
// confidence all touched across several events), item 6 approved.
export const items: KnowledgeItem[] = [
  item({ id: 'ki-escalation-runbook-096d9a2e502e-000001', item_type: 'claim', subject: 'Initial Notification', predicate: 'requires', object: 'On-call staff must confirm receipt of the alert within 15 minutes.', source_id: 'escalation-runbook-096d9a2e502e', section_id: 'section-escalation-runbook-096d9a2e502e-002', source_text: 'On-call staff must confirm receipt of the alert within 15 minutes.', start_pos: 47, end_pos: 113 }, 'approved'),
  item({ id: 'ki-escalation-runbook-096d9a2e502e-000002', item_type: 'claim', subject: 'Secondary Contact', predicate: 'should', object: 'Clinicians should route unresolved alerts to the secondary on-call physician.', source_id: 'escalation-runbook-096d9a2e502e', section_id: 'section-escalation-runbook-096d9a2e502e-003', source_text: 'Clinicians should route unresolved alerts to the secondary on-call physician.', start_pos: 137, end_pos: 214 }, 'rejected'),
  item({ id: 'ki-escalation-runbook-096d9a2e502e-000003', item_type: 'claim', subject: 'Vendor Escalation', predicate: 'records', object: 'The vendor liaison must record every escalation in the incident log.', source_id: 'escalation-runbook-096d9a2e502e', section_id: 'section-escalation-runbook-096d9a2e502e-004', source_text: 'The vendor liaison must record every escalation in the incident log.', start_pos: 238, end_pos: 306 }, 'deferred'),
  item({ id: 'ki-escalation-runbook-096d9a2e502e-000004', item_type: 'claim', subject: 'Legal Notification', predicate: 'should', object: 'Staff should confirm legal has been notified for any reportable event.', source_id: 'escalation-runbook-096d9a2e502e', section_id: 'section-escalation-runbook-096d9a2e502e-005', source_text: 'Staff should confirm legal has been notified for any reportable event.', start_pos: 331, end_pos: 401 }, 'approved'),
  item({ id: 'ki-escalation-runbook-096d9a2e502e-000005', item_type: 'claim', subject: 'Escalation Staff', predicate: 'confirms', object: 'Escalate within one hour,\tper policy,\rwith injected control characters.', source_id: 'escalation-runbook-096d9a2e502e', section_id: 'section-escalation-runbook-096d9a2e502e-006', source_text: 'Escalate within one hour, per policy.', start_pos: 429, end_pos: 505 }, 'edited'),
  item({ id: 'ki-escalation-runbook-096d9a2e502e-000006', item_type: 'claim', subject: 'Post-Incident Review', predicate: 'should', object: 'The safety committee should record findings within five business days.', source_id: 'escalation-runbook-096d9a2e502e', section_id: 'section-escalation-runbook-096d9a2e502e-007', source_text: 'The safety committee should record findings within five business days.', start_pos: 532, end_pos: 602 }, 'approved'),
];

// Copied verbatim (structure + text) from `.sopkb/reviews.json`, `<TS>` filled in with distinct mock times.
export const reviews: ReviewEvent[] = [
  { id: 'review-000001', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000001', action: 'approved', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:05:00Z', rationale: 'meets policy', previous_value: { review_status: 'proposed' }, new_value: { review_status: 'approved' } },
  { id: 'review-000002', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000002', action: 'rejected', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:07:00Z', rationale: 'does not apply here', previous_value: { review_status: 'proposed' }, new_value: { review_status: 'rejected' } },
  { id: 'review-000003', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000003', action: 'deferred', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:09:00Z', rationale: 'needs clinical sign-off', previous_value: { review_status: 'proposed' }, new_value: { review_status: 'deferred' } },
  { id: 'review-000004', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000004', action: 'approved', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:11:00Z', rationale: 'terminal before comment', previous_value: { review_status: 'proposed' }, new_value: { review_status: 'approved' } },
  { id: 'review-000005', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000004', action: 'commented', reviewer: 'local:reviewer-b', timestamp: '2026-07-20T14:13:00Z', rationale: 'confirmed with legal, no change needed', previous_value: null, new_value: null },
  { id: 'review-000006', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000005', action: 'edited', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:15:00Z', rationale: 'clarify subject', previous_value: { subject: 'Patient Communication' }, new_value: { subject: 'Escalation Staff' } },
  { id: 'review-000007', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000005', action: 'edited', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:17:00Z', rationale: 'clarify predicate', previous_value: { predicate: 'requires' }, new_value: { predicate: 'confirms' } },
  { id: 'review-000008', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000005', action: 'edited', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:19:00Z', rationale: 'inject CR/TAB for P-G7 Turtle-escaping coverage', previous_value: { object: 'Care coordinators must confirm the patient has been informed of the outcome.' }, new_value: { object: 'Escalate within one hour,\tper policy,\rwith injected control characters.' } },
  { id: 'review-000009', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000005', action: 'edited', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:21:00Z', rationale: 'align source text', previous_value: { source_text: 'Care coordinators must confirm the patient has been informed of the outcome.' }, new_value: { source_text: 'Escalate within one hour, per policy.' } },
  { id: 'review-000010', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000005', action: 'edited', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:23:00Z', rationale: 'reviewer confident', previous_value: { confidence: 0.82 }, new_value: { confidence: 0.95 } },
  { id: 'review-000011', knowledge_item_id: 'ki-escalation-runbook-096d9a2e502e-000006', action: 'approved', reviewer: 'local:reviewer-a', timestamp: '2026-07-20T14:25:00Z', rationale: 'approve before terminal-action test', previous_value: { review_status: 'proposed' }, new_value: { review_status: 'approved' } },
];

export const reportsMarkdown = {
  extraction_summary: `# Extraction Summary\n\n- Sources scanned: 1\n- Sections normalized: 7\n- Knowledge items mined: 6\n- Provider: fixture\n`,
  validation: `# Validation\n\nNo errors.\nNo warnings.\n`,
  conflicts: `# Conflicts\n\nNo conflicting knowledge items detected.\n`,
  freshness: `# Freshness\n\nAll sources are up to date.\n`,
  review_summary: `# Review Summary\n\n- Proposed: 0\n- Approved: 3\n- Rejected: 1\n- Deferred: 1\n- Edited: 1\n`,
  export_summary: `# Export Summary\n\n- graph_json: exports/${BUNDLE_KEY}/graph/graph.json\n- rdf: exports/${BUNDLE_KEY}/graph/triples.ttl\n`,
  agent_guide: `# Agent Guide\n\nThis bundle covers the incident escalation runbook: initial notification, secondary\ncontact routing, vendor escalation, legal notification, patient communication, and\npost-incident review.\n`,
};

export const agentTasks = [
  { id: 'escalation-response', title: 'Escalation response', description: 'What to do when an alert fires: notification, secondary contact, and vendor escalation steps.', matching_knowledge_count: 3 },
  { id: 'post-incident', title: 'Post-incident follow-up', description: 'Patient communication and post-incident review obligations after an escalation is resolved.', matching_knowledge_count: 2 },
];
