import { describe, expect, it } from 'vitest';
import { providerLabel } from './providerLabel';

describe('providerLabel', () => {
  it('labels fixture as "Offline (no network)", never the raw wire value', () => {
    expect(providerLabel('fixture')).toBe('Offline (no network)');
  });

  it('labels azure-llm with its configured profile name', () => {
    expect(providerLabel('azure-llm', 'My Profile')).toBe('azure-llm — My Profile');
  });

  it('falls back to a bare "azure-llm" label when no profile name is given', () => {
    expect(providerLabel('azure-llm')).toBe('azure-llm');
  });
});
