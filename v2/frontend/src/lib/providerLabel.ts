import type { MineProvider } from '../types/commands';

/**
 * Display label for a `MineProvider` in the Ingest screen's "Mining
 * provider" dropdown. The wire value `'fixture'` itself is deliberately
 * NOT renamed here — it's the literal string sent to the backend
 * (`mine_provider: "fixture"`), recorded verbatim in every mined item's
 * `metadata.provider`, and matches oss-launch's own real, current wire
 * value (`tools/sopkb/sopkb/normalize.py`'s `provider: str = "fixture"`
 * default). "fixture" is testing jargon that means nothing to an end user,
 * though, so only the text shown in the dropdown changes: "Offline (no
 * network)" keeps the exact framing already established as fine (no AI
 * involved, works with no network) without the confusing word.
 *
 * `azure-llm` keeps its own wire-value-prefixed label, appending the
 * configured profile's name so the option identifies which real credential
 * it will use — unchanged, since only "fixture" was reported as confusing.
 */
export function providerLabel(provider: MineProvider, profileName?: string): string {
  if (provider === 'fixture') {
    return 'Offline (no network)';
  }
  return profileName ? `azure-llm — ${profileName}` : 'azure-llm';
}
