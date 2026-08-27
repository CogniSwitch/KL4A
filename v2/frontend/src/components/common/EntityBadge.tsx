export type EntityKind =
  | 'source'
  | 'section'
  | 'concept'
  | 'knowledge_item'
  | 'evidence'
  | 'review'
  | 'relation'
  | 'decision_rule';

/**
 * One fixed color per OKF entity type, reused wherever that entity type is
 * rendered as a pill/badge — see the entity-color tokens defined in
 * `index.css` (`--color-source`/`--color-section`/etc.) and
 * docs/port/CATCHUP_PLAN.md, "ui/sopkb-web-redesign branch scan", idea #2.
 * Mirrors `StatusBadge`'s exact shape/sizing so the two badge families read
 * as one system — this one keyed by entity kind, `StatusBadge` by review
 * status; they are deliberately not merged since a single row can need
 * both (e.g. a relation's entity-kind badge next to its review-status one).
 */
const ENTITY_STYLES: Record<EntityKind, string> = {
  source: 'bg-source-soft text-source ring-source/30',
  section: 'bg-section-soft text-section ring-section/30',
  concept: 'bg-concept-soft text-concept ring-concept/30',
  knowledge_item: 'bg-knowledge-soft text-knowledge ring-knowledge/30',
  evidence: 'bg-evidence-soft text-evidence ring-evidence/30',
  review: 'bg-review-soft text-review ring-review/30',
  relation: 'bg-relation-soft text-relation ring-relation/30',
  decision_rule: 'bg-decision-rule-soft text-decision-rule ring-decision-rule/30',
};

const ENTITY_LABELS: Record<EntityKind, string> = {
  source: 'Source',
  section: 'Section',
  concept: 'Concept',
  knowledge_item: 'Knowledge',
  evidence: 'Evidence',
  review: 'Review',
  relation: 'Relation',
  decision_rule: 'Decision rule',
};

export function EntityBadge({ kind, label }: { kind: EntityKind; label?: string }) {
  return (
    <span
      className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ring-1 ring-inset ${ENTITY_STYLES[kind]}`}
    >
      {label ?? ENTITY_LABELS[kind]}
    </span>
  );
}
