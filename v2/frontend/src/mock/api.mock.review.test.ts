import { beforeEach, describe, expect, it } from 'vitest';
import * as mock from './api.mock';
import { __resetStoreForTests, store } from './store';
import { SopkbApiError } from '../types/commands';

/**
 * §4.5's core invariant: `comment_item` is the ONLY action legal on a
 * terminal (`approved`/`rejected`) item, and every other action must be
 * gated on the backend-computed `allowed_actions` from `get_review_detail`
 * — never a client-side re-derivation of "is this terminal?". These tests
 * exercise that gating at the mock-API boundary, which is exactly the
 * boundary every screen (in particular `ReviewScreen`) calls through.
 */
describe('review action gating (allowed_actions)', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('a fresh "proposed" item allows every action', async () => {
    const detail = await mock.get_review_detail('ki-follow-up-monitoring-procedure-38ef67176baf-000001');
    expect(detail.mutable).toBe(true);
    expect(detail.allowed_actions).toEqual(
      expect.arrayContaining(['approved', 'rejected', 'deferred', 'edited', 'commented']),
    );
  });

  it('approving an item collapses allowed_actions to comment-only', async () => {
    const itemId = 'ki-follow-up-monitoring-procedure-38ef67176baf-000001';
    await mock.approve_item(itemId, 'looks right');
    const detail = await mock.get_review_detail(itemId);
    expect(detail.item.review_status).toBe('approved');
    expect(detail.mutable).toBe(false);
    expect(detail.allowed_actions).toEqual(['commented']);
  });

  it('rejects a second status change on an approved item at the API layer, not just in the UI', async () => {
    const itemId = 'ki-follow-up-monitoring-procedure-38ef67176baf-000002';
    await mock.approve_item(itemId, 'first pass');
    await expect(mock.reject_item(itemId, 'changed my mind')).rejects.toBeInstanceOf(SopkbApiError);
    await expect(mock.defer_item(itemId, 'changed my mind')).rejects.toMatchObject({ kind: 'Conflict' });
    await expect(
      mock.edit_item(itemId, 'confidence', { kind: 'confidence', value: 0.5 }, 'nope'),
    ).rejects.toMatchObject({ kind: 'Conflict' });
  });

  it('comment_item is legal on an approved item and does not change review_status', async () => {
    const itemId = 'ki-follow-up-monitoring-procedure-38ef67176baf-000003';
    await mock.approve_item(itemId, 'approved first');
    const event = await mock.comment_item(itemId, 'confirmed with legal, no change needed');
    expect(event.action).toBe('commented');
    const detail = await mock.get_review_detail(itemId);
    expect(detail.item.review_status).toBe('approved'); // unchanged by the comment
  });

  it('edit_item rejects an unknown field before touching any state', async () => {
    const itemId = 'ki-follow-up-monitoring-procedure-38ef67176baf-000004';
    const before = await mock.list_review_events(itemId);
    await expect(
      mock.edit_item(itemId, 'not_a_real_field', { kind: 'text', value: 'x' }, 'oops'),
    ).rejects.toMatchObject({ kind: 'InvalidInput' });
    const after = await mock.list_review_events(itemId);
    expect(after).toEqual(before); // no reviews entry was created for the invalid field
  });

  it('an already-terminal escalation-runbook item (from the reviewed fixture) is comment-only from the start', async () => {
    store.selectedBundle = 'escalation-runbook';
    const detail = await mock.get_review_detail('ki-escalation-runbook-096d9a2e502e-000001');
    expect(detail.item.review_status).toBe('approved');
    expect(detail.allowed_actions).toEqual(['commented']);
  });
});
