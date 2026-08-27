import { beforeEach, describe, expect, it } from 'vitest';
import * as mock from './api.mock';
import { __resetStoreForTests } from './store';

const SECRET = 'sk-super-secret-value-must-never-round-trip';

/**
 * §4.8 non-negotiable: the API key must never cross the IPC boundary
 * outbound. `ProfileView` (returned by `get_settings` and `save_profile`)
 * has no field for it — only `has_api_key: boolean` — so this test proves
 * that guarantee holds through a real save/read round trip, not just that
 * the type says so.
 */
describe('settings: API key never round-trips', () => {
  beforeEach(() => {
    __resetStoreForTests();
  });

  it('save_profile never echoes the key back, only has_api_key', async () => {
    const saved = await mock.save_profile({
      name: 'Test profile',
      base_url: 'https://example.test',
      auth_style: 'api-key',
      model: 'gpt-test',
      max_output_tokens: 1024,
      timeout_seconds: 30,
      reasoning_effort: 'low',
      mining_prompt: 'p1',
      chat_prompt: 'p2',
      api_key: SECRET,
    });
    expect(saved.has_api_key).toBe(true);
    expect(JSON.stringify(saved)).not.toContain(SECRET);
    expect('api_key' in saved).toBe(false);
  });

  it('get_settings never contains any raw key value for any profile, ever', async () => {
    await mock.save_profile({
      name: 'Second profile',
      base_url: 'https://example.test',
      auth_style: 'api-key',
      model: 'gpt-test',
      max_output_tokens: 1024,
      timeout_seconds: 30,
      reasoning_effort: 'low',
      mining_prompt: 'p1',
      chat_prompt: 'p2',
      api_key: SECRET,
    });
    const settings = await mock.get_settings();
    expect(JSON.stringify(settings)).not.toContain(SECRET);
    for (const profile of settings.profiles) {
      expect('api_key' in profile).toBe(false);
    }
  });

  it('blank api_key on save means "keep existing", not "clear it"', async () => {
    const created = await mock.save_profile({
      name: 'Keeps its key',
      base_url: 'https://example.test',
      auth_style: 'api-key',
      model: 'gpt-test',
      max_output_tokens: 1024,
      timeout_seconds: 30,
      reasoning_effort: 'low',
      mining_prompt: 'p1',
      chat_prompt: 'p2',
      api_key: SECRET,
    });
    expect(created.has_api_key).toBe(true);

    // Re-save with no api_key field at all (what the Settings screen sends
    // when the user leaves the field blank) -- has_api_key must stay true.
    const resaved = await mock.save_profile({
      id: created.id,
      name: created.name,
      base_url: created.base_url,
      auth_style: created.auth_style,
      model: created.model,
      max_output_tokens: created.max_output_tokens,
      timeout_seconds: created.timeout_seconds,
      reasoning_effort: created.reasoning_effort,
      mining_prompt: created.mining_prompt,
      chat_prompt: created.chat_prompt,
    });
    expect(resaved.has_api_key).toBe(true);
  });
});
