import assert from 'node:assert/strict';
import test from 'node:test';
import { validateInitialPublish } from './initial-publish-context.mjs';

const sha = 'a'.repeat(40);
const metadata = { packages: [{ name: 'stack-compiler', version: '0.1.0', publish: ['crates-io'], license: 'Apache-2.0', rust_version: '1.85' }] };
const runs = [{ headSha: sha, status: 'completed', conclusion: 'success' }];

test('accepts only the initial package and successful exact-commit CI', () => {
  validateInitialPublish(metadata, runs, sha);
});

test('rejects missing, stale, running, failed, and skipped CI', () => {
  for (const invalid of [[], [...runs, ...runs], [{ ...runs[0], headSha: 'b'.repeat(40) }], [{ ...runs[0], status: 'in_progress' }], [{ ...runs[0], conclusion: 'failure' }], [{ ...runs[0], conclusion: 'skipped' }]]) {
    assert.throws(() => validateInitialPublish(metadata, invalid, sha));
  }
});

test('rejects changed package identity, registry, version, license, and MSRV', () => {
  for (const change of [{ name: 'other' }, { version: '0.1.1' }, { publish: null }, { publish: ['other-registry'] }, { license: 'MIT' }, { rust_version: '1.86' }]) {
    assert.throws(() => validateInitialPublish({ packages: [{ ...metadata.packages[0], ...change }] }, runs, sha));
  }
  assert.throws(() => validateInitialPublish({ packages: [] }, runs, sha));
  assert.throws(() => validateInitialPublish({ packages: [...metadata.packages, ...metadata.packages] }, runs, sha));
});

test('rejects mutable, malformed, and shell-like commit inputs', () => {
  for (const invalid of ['main', 'a'.repeat(39), 'A'.repeat(40), `${sha}\n`, '$(echo unsafe)', undefined]) {
    assert.throws(() => validateInitialPublish(metadata, runs, invalid));
  }
});
