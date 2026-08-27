import { Link } from 'react-router-dom';
import * as api from '../lib/api';
import { useAsync, useBundleInvalidation } from '../lib/hooks';
import { LoadingBlock, ErrorBanner, EmptyState } from '../components/common/Feedback';

export function ConceptsScreen() {
  const concepts = useAsync(() => api.get_concept_index(), []);
  useBundleInvalidation(() => concepts.reload(), ['items', 'reviews']);

  if (concepts.loading) return <LoadingBlock label="Loading concepts…" />;
  if (concepts.error) return <ErrorBanner message="Could not load concepts" detail={concepts.error} />;
  if (!concepts.data || concepts.data.length === 0) {
    return (
      <EmptyState>
        <p>No concepts derived yet.</p>
        <Link to="/ingest" className="mt-2 inline-block text-accent underline">
          Go to Ingest to add and mine sources
        </Link>
      </EmptyState>
    );
  }

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold text-ink">Concepts</h1>
      <ul className="grid grid-cols-2 gap-3">
        {concepts.data.map((concept) => (
          <li key={concept.id} className="rounded-lg border border-line bg-panel p-4">
            <Link to={`/concepts/${concept.id}`} className="font-medium text-ink hover:underline">
              {concept.label}
            </Link>
            <p className="mt-1 text-xs text-muted">
              {concept.item_count} item{concept.item_count === 1 ? '' : 's'} · {concept.rule_count} rule
              {concept.rule_count === 1 ? '' : 's'}
            </p>
            <div className="mt-2 flex flex-wrap gap-1.5">
              {Object.entries(concept.statuses).map(([status, count]) => (
                <span key={status} className="rounded-full bg-panel-muted px-2 py-0.5 text-xs text-muted">
                  {status}: {count}
                </span>
              ))}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
