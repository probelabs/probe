import assert from 'node:assert/strict';
import test from 'node:test';
import { deniedOutput, successfulOutput } from '../manual/governed-codex-isolated-writer-live.mjs';

const observedBOutput = 'Script completed\nWall time 0.1 seconds\nOutput:\n\n{}';
const observedADenial = 'Script failed\nWall time 0.0 seconds\nOutput:\n\nScript error:\npatch rejected: writing outside of the project; rejected by user approval settings';

test('isolated-writer recorder accepts the observed native exec success envelope', () => {
  assert.equal(successfulOutput(observedBOutput), true);
  assert.equal(deniedOutput(observedBOutput), false);
});

test('isolated-writer recorder preserves denial and rejects unapproved output shapes', () => {
  assert.equal(deniedOutput(observedADenial), true);
  assert.equal(successfulOutput(observedADenial), false);
  for (const output of ['', '{}', '[{}]', '[{"ok":true}]', 'Script completed\nWall time 0.1 seconds\nOutput:\n\n{"unexpected":true}', 'Script failed\nWall time 0.0 seconds\nOutput:\n\n{}']) {
    assert.equal(successfulOutput(output), false, output);
  }
});
