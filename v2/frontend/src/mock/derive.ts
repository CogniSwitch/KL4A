/**
 * Helpers that derive Evidence/Relation/DecisionRule/Concept mock records
 * from a bundle's `KnowledgeItem[]`, the same way the real backend derives
 * them from `items.json` (see `sopkb-rust/fixtures/cases/reference/expected-python/bundle/{relations,rules,concepts}`
 * for the real on-disk shapes this mirrors).
 *
 * This is NOT a reimplementation of the Rust derivation logic — it exists
 * only so the hand-written fixture data in `src/mock/fixtures/*.ts` can stay
 * small (just sources/sections/items, which is what a human would actually
 * author) while still producing believable, internally-consistent
 * Evidence/Relation/Concept data for every screen. Field values (ids, slugs,
 * labels) are deterministic functions of the source item so the same mock
 * item always derives the same relation/evidence id across reloads.
 */
import type {
  Concept,
  DecisionRule,
  Evidence,
  KnowledgeItem,
  Relation,
} from '../types/commands';

export function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 60)
    .replace(/-+$/g, '');
}

export function evidenceIdForItem(itemId: string): string {
  return `evidence-${itemId}`;
}

export function relationIdForItem(itemId: string): string {
  return `kr-${itemId}`;
}

export function ruleIdForItem(itemId: string): string {
  return `rule-${itemId}-${slugify(itemId)}`;
}

export function conceptIdForSubject(subject: string): string {
  return `concept-${slugify(subject)}`;
}

export function deriveEvidence(item: KnowledgeItem): Evidence {
  return {
    knowledge_item_id: item.id,
    source_id: item.source_id,
    section_id: item.section_id,
    source_text: item.source_text,
    span_status: item.span_status,
    start_pos: item.start_pos,
    end_pos: item.end_pos,
  };
}

export function deriveRelation(item: KnowledgeItem): Relation {
  const conceptId = conceptIdForSubject(item.subject);
  const objectSlug = slugify(item.object.slice(0, 60));
  return {
    id: relationIdForItem(item.id),
    type: 'Knowledge Relation',
    subject: {
      id: conceptId,
      label: item.subject,
      text: item.subject,
      okf_path: `concepts/${conceptId}.md`,
    },
    predicate: {
      id: `predicate-${item.predicate}`,
      text: item.predicate,
    },
    object: {
      id: `object-${objectSlug}`,
      text: item.object,
      label: item.object.length > 72 ? `${item.object.slice(0, 72)}…` : item.object,
    },
    knowledge_piece_id: item.id,
    evidence_id: evidenceIdForItem(item.id),
    review_status: item.review_status,
    confidence: item.confidence,
    rdf_compatible: true,
  };
}

const ACTION_BY_PREDICATE: Record<string, string> = {
  requires: 'require',
  should: 'recommend',
  records: 'record',
  confirms: 'confirm',
};

/**
 * Only "obligation-shaped" items (roughly: predicates that read as a duty)
 * get a decision rule, mirroring how the real bundle's `rules/` directory
 * has one rule per knowledge item but not every item type would sensibly
 * produce one. For this mock corpus every item qualifies, matching the
 * reference fixture where all 16 items have a corresponding rule file.
 */
export function deriveRule(item: KnowledgeItem, opts?: { condition?: DecisionRule['condition'] }): DecisionRule {
  const action = ACTION_BY_PREDICATE[item.predicate] ?? 'flag';
  return {
    id: ruleIdForItem(item.id),
    type: 'SOP Decision Rule',
    title: item.object.length > 80 ? `${item.object.slice(0, 80)}…` : item.object,
    knowledge_item_id: item.id,
    source_id: item.source_id,
    section_id: item.section_id,
    review_status: item.review_status,
    confidence: item.confidence,
    condition: opts?.condition,
    obligation: {
      fact: `${slugify(item.subject)}_${action}`,
      action,
      label: item.object,
    },
    evidence_id: evidenceIdForItem(item.id),
    relation_id: relationIdForItem(item.id),
    okf_path: `rules/${ruleIdForItem(item.id)}.md`,
    otherwise: opts?.condition
      ? {
          action_required: false,
          label: `Not required when ${opts.condition.label.toLowerCase()} does not hold.`,
        }
      : undefined,
  };
}

export function deriveConcepts(items: KnowledgeItem[], rules: DecisionRule[]): Concept[] {
  const bySubject = new Map<string, KnowledgeItem[]>();
  for (const item of items) {
    const list = bySubject.get(item.subject) ?? [];
    list.push(item);
    bySubject.set(item.subject, list);
  }
  const concepts: Concept[] = [];
  for (const [subject, subjectItems] of bySubject) {
    const conceptId = conceptIdForSubject(subject);
    const ruleCount = rules.filter((r) => subjectItems.some((i) => i.id === r.knowledge_item_id)).length;
    const statuses: Partial<Record<KnowledgeItem['review_status'], number>> = {};
    for (const item of subjectItems) {
      statuses[item.review_status] = (statuses[item.review_status] ?? 0) + 1;
    }
    concepts.push({
      id: conceptId,
      label: subject,
      item_count: subjectItems.length,
      rule_count: ruleCount,
      statuses,
    });
  }
  return concepts.sort((a, b) => a.label.localeCompare(b.label));
}
