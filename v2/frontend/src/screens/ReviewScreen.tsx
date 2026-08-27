import { useState } from 'react';
import type { ReactNode } from 'react';
import { Link, useParams } from 'react-router-dom';
import * as api from '../lib/api';
import { useAsync, useBundleInvalidation } from '../lib/hooks';
import { LoadingBlock, ErrorBanner } from '../components/common/Feedback';
import { StatusBadge } from '../components/common/StatusBadge';
import { EntityBadge } from '../components/common/EntityBadge';
import { ConfidenceMeter } from '../components/common/ConfidenceMeter';
import { PredicatePill } from '../components/common/PredicatePill';
import { Button } from '../components/common/Button';
import { Input, Textarea } from '../components/common/Input';
import type { ReviewAction } from '../types/commands';

/** Plain-language explanation of what each review action actually does — surfaced
 * directly in the UI (previously only knowable by reading the backend's own state
 * machine) so "what happens if I click this" doesn't require guessing. */
const ACTION_LEGEND: Record<ReviewAction, string> = {
  approved: 'Final decision — trustworthy. Locks the item; only a comment can still be added after.',
  rejected: 'Final decision — not usable. Locks the item, but nothing is deleted; only a comment can still be added after.',
  deferred: 'Not ready to decide — stays open for a future Approve/Reject. Can be deferred again as many times as needed.',
  commented: 'Adds a note to the history without changing the decision. The only action still available once approved or rejected.',
  edited: 'Corrects a value and reopens the item for a fresh Approve/Reject decision.',
};

/**
 * The interactive review screen. Every status-changing action
 * (approve/reject/defer/edit) is gated on `allowed_actions`, fetched from
 * `get_review_detail` — never re-derived client-side from
 * `item.review_status`, per §4.5's explicit warning that the two sides of
 * the app must not silently drift on what "terminal" means. `comment` is
 * always offered, even on a terminal item — it is the one action legal
 * there (C §1).
 *
 * Predicate/Confidence render via `PredicatePill`/`ConfidenceMeter`
 * (CATCHUP_PLAN.md's "ui/sopkb-web-redesign branch scan", idea 5) instead
 * of plain text / a bare `toFixed(2)` number, matching the same two
 * components on the Knowledge screen's table.
 */
export function ReviewScreen() {
  const { itemId } = useParams<{ itemId: string }>();
  const detail = useAsync(() => api.get_review_detail(itemId!), [itemId]);
  const events = useAsync(() => api.list_review_events(itemId), [itemId]);
  useBundleInvalidation(() => {
    detail.reload();
    events.reload();
  }, ['items', 'reviews']);

  const [rationale, setRationale] = useState('');
  const [busyAction, setBusyAction] = useState<ReviewAction | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionSuccess, setActionSuccess] = useState<string | null>(null);

  // Which single field (if any) is currently showing an inline editor in the
  // detail card below, instead of its static value — `edit_item` only ever
  // commits one field per call, so editing stays one-field-at-a-time, but the
  // affordance to start editing now lives directly on the value itself
  // (click "Edit" next to Subject/Predicate/Object/Source text/Confidence)
  // instead of a disconnected field-picker section further down the page.
  const [editingField, setEditingField] = useState<string | null>(null);
  const [editText, setEditText] = useState('');
  const [editConfidence, setEditConfidence] = useState(0.8);

  if (detail.loading) return <LoadingBlock label="Loading review detail…" />;
  if (detail.error) return <ErrorBanner message="Could not load review detail" detail={detail.error} />;
  if (!detail.data) return null;

  const { item, relation, rules, evidence_id, allowed_actions } = detail.data;
  const can = (action: ReviewAction) => allowed_actions.includes(action);

  async function runAction(action: ReviewAction, fn: () => Promise<unknown>, successMessage: string) {
    setBusyAction(action);
    setActionError(null);
    setActionSuccess(null);
    try {
      await fn();
      setRationale('');
      await detail.reload();
      await events.reload();
      setActionSuccess(successMessage);
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyAction(null);
    }
  }

  function startEditing(field: string) {
    setEditingField(field);
    setActionError(null);
    setActionSuccess(null);
    if (field === 'confidence') {
      setEditConfidence(item.confidence);
    } else {
      setEditText(String((item as unknown as Record<string, unknown>)[field] ?? ''));
    }
  }

  async function handleFieldSave(field: string) {
    const value = field === 'confidence' ? { kind: 'confidence' as const, value: editConfidence } : { kind: 'text' as const, value: editText };
    await runAction('edited', () => api.edit_item(item.id, field, value, rationale || 'edited via UI'), 'Saved.');
    setEditingField(null);
  }

  return (
    <div className="space-y-6">
      <div>
        <Link to="/knowledge" className="text-sm text-muted underline">
          ← Back to knowledge
        </Link>
        <div className="mt-1 flex items-center gap-3">
          <h1 className="text-xl font-semibold text-ink">{item.id}</h1>
          <StatusBadge status={item.review_status} />
        </div>
      </div>

      <section className="rounded-lg border border-line bg-panel p-4">
        <dl className="grid grid-cols-3 gap-3 text-sm">
          <EditableDetailField
            label="Subject"
            field="subject"
            span={1}
            editable={can('edited')}
            editing={editingField === 'subject'}
            busy={busyAction === 'edited' && editingField === 'subject'}
            onEdit={() => startEditing('subject')}
            onCancel={() => setEditingField(null)}
            onSave={() => void handleFieldSave('subject')}
            editValue={editText}
            onEditValueChange={setEditText}
          >
            <span className="text-ink">{item.subject}</span>
          </EditableDetailField>

          <EditableDetailField
            label="Predicate"
            field="predicate"
            span={1}
            editable={can('edited')}
            editing={editingField === 'predicate'}
            busy={busyAction === 'edited' && editingField === 'predicate'}
            onEdit={() => startEditing('predicate')}
            onCancel={() => setEditingField(null)}
            onSave={() => void handleFieldSave('predicate')}
            editValue={editText}
            onEditValueChange={setEditText}
          >
            <PredicatePill predicate={item.predicate} />
          </EditableDetailField>

          <EditableDetailField
            label="Confidence"
            field="confidence"
            span={1}
            editable={can('edited')}
            editing={editingField === 'confidence'}
            busy={busyAction === 'edited' && editingField === 'confidence'}
            onEdit={() => startEditing('confidence')}
            onCancel={() => setEditingField(null)}
            onSave={() => void handleFieldSave('confidence')}
            confidenceValue={editConfidence}
            onConfidenceValueChange={setEditConfidence}
          >
            <ConfidenceMeter value={item.confidence} />
          </EditableDetailField>

          <EditableDetailField
            label="Object"
            field="object"
            span={3}
            editable={can('edited')}
            editing={editingField === 'object'}
            busy={busyAction === 'edited' && editingField === 'object'}
            onEdit={() => startEditing('object')}
            onCancel={() => setEditingField(null)}
            onSave={() => void handleFieldSave('object')}
            editValue={editText}
            onEditValueChange={setEditText}
            multiline
          >
            <span className="text-ink">{item.object}</span>
          </EditableDetailField>

          <EditableDetailField
            label={`Source text (evidence ${evidence_id})`}
            field="source_text"
            span={3}
            editable={can('edited')}
            editing={editingField === 'source_text'}
            busy={busyAction === 'edited' && editingField === 'source_text'}
            onEdit={() => startEditing('source_text')}
            onCancel={() => setEditingField(null)}
            onSave={() => void handleFieldSave('source_text')}
            editValue={editText}
            onEditValueChange={setEditText}
            multiline
          >
            <span className="text-ink/80">{item.source_text}</span>
          </EditableDetailField>
        </dl>
      </section>

      {relation && (
        <section className="rounded-lg border border-line bg-panel p-4 text-sm">
          <h2 className="text-sm font-semibold text-ink">Relation</h2>
          <p className="mt-1 text-ink/80">
            <span className="font-medium">{relation.subject.label}</span> {relation.predicate.text}{' '}
            {relation.object.label}
          </p>
        </section>
      )}

      {rules.length > 0 && (
        <section className="rounded-lg border border-line bg-panel p-4 text-sm">
          <h2 className="text-sm font-semibold text-ink">Decision rules ({rules.length})</h2>
          <ul className="mt-2 space-y-2">
            {rules.map((rule) => (
              <li key={rule.id} className="flex items-center gap-2 text-ink/80">
                <EntityBadge kind="decision_rule" />
                {rule.title}
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="rounded-lg border border-line bg-panel p-4">
        <h2 className="text-sm font-semibold text-ink">Review actions</h2>
        <p className="mt-1 text-xs text-muted-soft">
          {ACTION_LEGEND.approved} · {ACTION_LEGEND.rejected} · {ACTION_LEGEND.deferred} · {ACTION_LEGEND.commented}
        </p>
        <Textarea
          value={rationale}
          onChange={(e) => setRationale(e.target.value)}
          placeholder="Rationale…"
          rows={2}
          className="mt-2 w-full"
        />

        {actionError && <p className="mt-2 text-sm text-bad">{actionError}</p>}
        {!actionError && actionSuccess && <p className="mt-2 text-sm text-ok">{actionSuccess}</p>}

        <div className="mt-3 flex flex-wrap gap-2">
          <ActionButton
            label="Approve"
            title={ACTION_LEGEND.approved}
            enabled={can('approved')}
            busy={busyAction === 'approved'}
            onClick={() => void runAction('approved', () => api.approve_item(item.id, rationale || 'approved via UI'), 'Approved.')}
          />
          <ActionButton
            label="Reject"
            title={ACTION_LEGEND.rejected}
            enabled={can('rejected')}
            busy={busyAction === 'rejected'}
            onClick={() => void runAction('rejected', () => api.reject_item(item.id, rationale || 'rejected via UI'), 'Rejected.')}
          />
          <ActionButton
            label="Defer"
            title={ACTION_LEGEND.deferred}
            enabled={can('deferred')}
            busy={busyAction === 'deferred'}
            onClick={() => void runAction('deferred', () => api.defer_item(item.id, rationale || 'deferred via UI'), 'Deferred.')}
          />
          <ActionButton
            label="Comment"
            title={ACTION_LEGEND.commented}
            enabled={can('commented') && rationale.trim().length > 0}
            busy={busyAction === 'commented'}
            onClick={() => void runAction('commented', () => api.comment_item(item.id, rationale || 'comment via UI'), 'Comment added.')}
          />
        </div>
        {!can('approved') && (
          <p className="mt-2 text-xs text-muted-soft">
            This item is {item.review_status}; only commenting is allowed on a terminal item.
          </p>
        )}
      </section>

      <section>
        <h2 className="text-sm font-semibold text-ink">Review history</h2>
        {events.data && events.data.length === 0 && <p className="mt-1 text-sm text-muted">No review events yet.</p>}
        <ul className="mt-2 space-y-2">
          {events.data
            ?.slice()
            .reverse()
            .map((event) => (
              <li key={event.id} className="rounded-lg border border-line bg-panel px-4 py-2.5 text-sm">
                <div className="flex items-center justify-between text-xs text-muted-soft">
                  <EntityBadge kind="review" label={`${event.action} · ${event.reviewer}`} />
                  <span>{new Date(event.timestamp).toLocaleString()}</span>
                </div>
                <p className="mt-1 text-ink/80">{event.rationale}</p>
              </li>
            ))}
        </ul>
      </section>
    </div>
  );
}

function ActionButton({
  label,
  title,
  enabled,
  busy,
  onClick,
}: {
  label: string;
  title?: string;
  enabled: boolean;
  busy: boolean;
  onClick: () => void;
}) {
  return (
    <Button variant="secondary" title={title} disabled={!enabled || busy} onClick={onClick}>
      {busy ? `${label}…` : label}
    </Button>
  );
}

/**
 * One row of the detail card: shows `children` (the current, formatted value)
 * by default, or — when `editable` and `editing` — an inline input replacing
 * it, with Save/Cancel right there. Keeps the "click Edit next to the value
 * you want to change" affordance directly on the card instead of a
 * disconnected field-picker section (the old design this replaces).
 */
function EditableDetailField({
  label,
  field: _field,
  span,
  editable,
  editing,
  busy,
  onEdit,
  onCancel,
  onSave,
  editValue,
  onEditValueChange,
  confidenceValue,
  onConfidenceValueChange,
  multiline,
  children,
}: {
  label: string;
  field: string;
  span: 1 | 3;
  editable: boolean;
  editing: boolean;
  busy: boolean;
  onEdit: () => void;
  onCancel: () => void;
  onSave: () => void;
  editValue?: string;
  onEditValueChange?: (value: string) => void;
  confidenceValue?: number;
  onConfidenceValueChange?: (value: number) => void;
  multiline?: boolean;
  children: ReactNode;
}) {
  const isConfidence = confidenceValue !== undefined;
  return (
    <div className={span === 3 ? 'col-span-3' : undefined}>
      <div className="flex items-center justify-between gap-2">
        <dt className="text-xs uppercase text-muted-soft">{label}</dt>
        {editable && !editing && (
          <button type="button" onClick={onEdit} className="text-xs text-accent underline">
            Edit
          </button>
        )}
      </div>
      {!editing && <dd className="mt-0.5">{children}</dd>}
      {editing && (
        <dd className="mt-1 space-y-1.5">
          {isConfidence ? (
            <Input
              type="number"
              min={0}
              max={1}
              step={0.01}
              value={confidenceValue}
              onChange={(e) => onConfidenceValueChange?.(Number(e.target.value))}
              className="w-32"
            />
          ) : multiline ? (
            <Textarea value={editValue} onChange={(e) => onEditValueChange?.(e.target.value)} rows={3} className="w-full" />
          ) : (
            <Input type="text" value={editValue} onChange={(e) => onEditValueChange?.(e.target.value)} className="w-full" />
          )}
          <div className="flex gap-2">
            <Button variant="primary" disabled={busy || (!isConfidence && !editValue?.trim())} onClick={onSave} className="!py-1 !text-xs">
              {busy ? 'Saving…' : 'Save'}
            </Button>
            <Button variant="ghost" disabled={busy} onClick={onCancel} className="!py-1 !text-xs">
              Cancel
            </Button>
          </div>
        </dd>
      )}
    </div>
  );
}
