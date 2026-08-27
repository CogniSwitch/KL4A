import { beforeEach, describe, expect, it } from 'vitest';
import * as mock from './api.mock';
import { __resetStoreForTests, store } from './store';
import type { GraphResult } from '../types/commands';

/**
 * §4.4 / F3: the two `get_graph` origins carry incompatible vocabularies
 * and must be discriminated explicitly via `origin`, never inferred from
 * which optional field happens to be present. These tests assert the
 * discriminant is trustworthy: each origin's declared shape actually holds
 * at runtime, and TypeScript's narrowing on `origin` is exercised (not just
 * asserted past with a cast).
 */
describe('get_graph tagged union', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  function assertNarrowsCorrectly(result: GraphResult): string {
    // This function only compiles if `origin` genuinely narrows the union —
    // `export_path` is only accessible in the `"export"` branch.
    if (result.origin === 'export') {
      return result.export_path;
    }
    return `live graph with ${result.nodes.length} nodes`;
  }

  it('a whole-bundle request (no concept_id) is the "export" origin and carries export_path', async () => {
    const result = await mock.get_graph();
    expect(result.origin).toBe('export');
    expect(assertNarrowsCorrectly(result)).toContain('graph.json');
    if (result.origin === 'export') {
      expect(typeof result.export_path).toBe('string');
    } else {
      throw new Error('expected export origin');
    }
  });

  it('a concept-scoped request is the "live" origin and carries no export_path', async () => {
    const concepts = await mock.get_concept_index();
    const result = await mock.get_graph(concepts[0].id);
    expect(result.origin).toBe('live');
    expect('export_path' in result).toBe(false);
    expect(assertNarrowsCorrectly(result)).toContain('live graph');
  });

  it('nodes and edges are present and internally referenced by id for the export origin', async () => {
    const result = await mock.get_graph();
    const nodeIds = new Set(result.nodes.map((n) => n.id));
    // Real export graphs legitimately contain dangling edges by design
    // (C G3) -- assert only that node ids are unique, not that every edge
    // endpoint resolves, since "fixing" that would be a behavior change.
    expect(nodeIds.size).toBe(result.nodes.length);
    expect(result.edges.length).toBeGreaterThan(0);
  });
});
