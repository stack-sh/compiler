import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

export function validateInitialPublish(metadata, runs, expectedSha) {
  assert.match(expectedSha, /^[a-f0-9]{40}$/);
  assert.equal(metadata.packages.length, 1, 'Expected one source package');
  const crate = metadata.packages[0];
  assert.equal(crate.name, 'stack-compiler');
  assert.equal(crate.version, '0.1.0', 'Only the initial version may use this workflow');
  assert.deepEqual(crate.publish, ['crates-io']);
  assert.equal(crate.license, 'Apache-2.0');
  assert.equal(crate.rust_version, '1.85');
  assert.equal(runs.length, 1, 'The exact main commit needs a CI run');
  assert.equal(runs[0].headSha, expectedSha);
  assert.equal(runs[0].status, 'completed');
  assert.equal(runs[0].conclusion, 'success');
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const metadata = JSON.parse(await readFile(process.argv[2], 'utf8'));
  const runs = JSON.parse(await readFile(process.argv[3], 'utf8'));
  validateInitialPublish(metadata, runs, process.env.EXPECTED_SHA);
  console.log('Initial package identity and exact-commit CI verified.');
}
