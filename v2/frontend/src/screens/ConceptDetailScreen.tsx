import { Link, useParams } from 'react-router-dom';
import * as api from '../lib/api';
import { useAsync } from '../lib/hooks';
import { LoadingBlock, ErrorBanner } from '../components/common/Feedback';
import { StatusBadge } from '../components/common/StatusBadge';

/** 24 rules/concept cap (§4.4) — a genuine backend param, so we just render whatever comes back and note the cap. */
export function ConceptDetailScreen() {
  const { conceptId } = useParams<{ conceptId: string }>();
  const detail = useAsync(() => api.get_concept_detail(conceptId!), [conceptId]);

  if (detail.loading) return <LoadingBlock label="Loading concept…" />;
  if (detail.error) return <ErrorBanner message="Could not load concept" detail={detail.error} />;
  if (!detail.data) return null;

  const { concept, items, rules } = detail.data;

  return (
    <div className="space-y-6">
      <div>
        <Link to="/concepts" className="text-sm text-muted underline">
          ← Back to concepts
        </Link>
        <h1 className="mt-1 text-xl font-semibold text-ink">{concept.label}</h1>
      </div>

      <section>
        <h2 className="text-sm font-semibold text-ink">Knowledge items ({items.length})</h2>
        <ul className="mt-2 divide-y divide-line rounded-lg border border-line bg-panel">
          {items.map((item) => (
            <li key={item.id} className="flex items-center justify-between px-4 py-2.5 text-sm">
              <div>
                <span className="font-medium text-ink">{item.predicate}</span>{' '}
                <span className="text-muted">{item.object}</span>
              </div>
              <div className="flex items-center gap-3">
                <StatusBadge status={item.review_status} />
                <Link to={`/review/${item.id}`} className="text-accent underline">
                  Review
                </Link>
              </div>
            </li>
          ))}
        </ul>
      </section>

      <section>
        <h2 className="text-sm font-semibold text-ink">
          Decision rules ({rules.length}
          {rules.length >= 24 ? ', capped at 24' : ''})
        </h2>
        <ul className="mt-2 divide-y divide-line rounded-lg border border-line bg-panel">
          {rules.map((rule) => (
            <li key={rule.id} className="px-4 py-2.5 text-sm">
              <p className="font-medium text-ink">{rule.title}</p>
              {rule.condition && (
                <p className="mt-0.5 text-xs text-muted">If {rule.condition.label.toLowerCase()}: {rule.obligation.label}</p>
              )}
              {!rule.condition && <p className="mt-0.5 text-xs text-muted">{rule.obligation.label}</p>}
            </li>
          ))}
          {rules.length === 0 && <li className="px-4 py-4 text-sm text-muted">No decision rules for this concept.</li>}
        </ul>
      </section>
    </div>
  );
}
