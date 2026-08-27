import { beforeEach, describe, expect, it, vi } from 'vitest';
import { listen } from '@tauri-apps/api/event';
import { publish, subscribeMock, subscribeReal, __resetAllListeners } from './events';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

const mockListen = listen as unknown as ReturnType<typeof vi.fn>;

/**
 * `src/lib/events.ts` picks one of two backends for `subscribe()` based on
 * `USE_MOCK` (always true in this jsdom/vitest environment, since there's no
 * real Tauri webview). `subscribeReal`/`subscribeMock` are exported
 * specifically so both backends can be exercised directly here, regardless
 * of which one the module-load-time `USE_MOCK` flag actually picked.
 */
describe('events: mock backend (in-memory pub/sub)', () => {
  beforeEach(() => {
    __resetAllListeners();
  });

  it('delivers a published payload to a subscribed handler', () => {
    const handler = vi.fn();
    subscribeMock('bundle://state-changed', handler);
    publish('bundle://state-changed', { key: 'b1', scope: ['items', 'reviews', 'okf'] });
    expect(handler).toHaveBeenCalledWith({ key: 'b1', scope: ['items', 'reviews', 'okf'] });
  });

  it('delivers to every subscriber, not just the first', () => {
    const a = vi.fn();
    const b = vi.fn();
    subscribeMock('settings://changed', a);
    subscribeMock('settings://changed', b);
    publish('settings://changed', {});
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledTimes(1);
  });

  it('stops delivering once unsubscribed', () => {
    const handler = vi.fn();
    const unsubscribe = subscribeMock('agent://progress', handler);
    unsubscribe();
    publish('agent://progress', { phase: 'done', detail: 'x' });
    expect(handler).not.toHaveBeenCalled();
  });

  it('a handler that unsubscribes itself mid-dispatch does not break delivery to the other handler', () => {
    const other = vi.fn();
    let unsubscribeSelf: () => void = () => {};
    const self = vi.fn(() => unsubscribeSelf());
    unsubscribeSelf = subscribeMock('ingest://progress', self);
    subscribeMock('ingest://progress', other);
    publish('ingest://progress', { step: 'scan', status: 'done', detail: '' });
    expect(self).toHaveBeenCalledTimes(1);
    expect(other).toHaveBeenCalledTimes(1);
  });
});

describe('events: real backend (@tauri-apps/api/event listen() wrapper)', () => {
  beforeEach(() => {
    mockListen.mockReset();
  });

  it('unwraps event.payload before handing it to the handler', async () => {
    let deliver: (event: { payload: unknown }) => void = () => {};
    mockListen.mockImplementation((_name: string, cb: (event: { payload: unknown }) => void) => {
      deliver = cb;
      return Promise.resolve(vi.fn());
    });
    const handler = vi.fn();
    subscribeReal('bundle://index-changed', handler);
    await Promise.resolve(); // let listen()'s promise settle
    deliver({ payload: { bundles_root: '/root' } });
    expect(handler).toHaveBeenCalledWith({ bundles_root: '/root' });
  });

  it('calling the returned unsubscribe after listen() resolves calls the real unlisten immediately', async () => {
    const unlisten = vi.fn();
    mockListen.mockResolvedValue(unlisten);
    const unsubscribe = subscribeReal('settings://changed', vi.fn());
    await Promise.resolve();
    await Promise.resolve();
    unsubscribe();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('calling unsubscribe BEFORE listen() resolves defers, then still unlistens once registration completes', async () => {
    const unlisten = vi.fn();
    let resolveListen: (fn: () => void) => void = () => {};
    mockListen.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveListen = resolve;
        }),
    );
    const unsubscribe = subscribeReal('agent://progress', vi.fn());
    unsubscribe(); // called before listen() has resolved
    expect(unlisten).not.toHaveBeenCalled();
    resolveListen(unlisten);
    await Promise.resolve();
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
