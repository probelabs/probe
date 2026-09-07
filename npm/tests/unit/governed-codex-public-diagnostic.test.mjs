import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { publicDiagnosticFailure, publicDiagnosticPlan } from '../manual/governed-codex-isolated-writer-live.mjs';

const scriptSource = await readFile(new URL('../manual/governed-codex-isolated-writer-live.mjs', import.meta.url), 'utf8');

test('public diagnostic plan is bounded to Proof CLI and does not enter private rollout inspection', () => {
  const plan = publicDiagnosticPlan('/private/tmp/proof');
  assert.equal(plan.mode, 'public-proof-cli-diagnostic');
  assert.deepEqual(plan.proofNew, [
    'req', 'new', 'specs/system', '--component', 'writer_b', '--description',
    'The isolated writer shall preserve its B sentinel', '--format', 'json',
  ]);
  assert.deepEqual(plan.proofList, ['req', 'list', '--format', 'json']);
  assert.deepEqual(plan.proofShow, ['req', 'show', '<id>', '--with', 'file', '--format', 'json']);
  assert.equal(plan.privateRolloutInspection, false);
  assert.throws(() => publicDiagnosticPlan('proof'), /absolute/);

  const publicBodyStart = scriptSource.indexOf('async function runPublicDiagnostic');
  const publicBodyEnd = scriptSource.indexOf('\nasync function parseRollout', publicBodyStart);
  assert.ok(publicBodyStart >= 0 && publicBodyEnd > publicBodyStart);
  const publicBody = scriptSource.slice(publicBodyStart, publicBodyEnd);
  for (const forbidden of ['snapshotRollouts(', 'rolloutEvidence(', 'parseRollout(', 'createReadStream(', 'readdir('])
    assert.equal(publicBody.includes(forbidden), false, forbidden);
  const diagnosticDispatch = scriptSource.indexOf(`process.env[PUBLIC_DIAGNOSTIC_ENV] === '1'`);
  const legacyPrivateCall = scriptSource.indexOf('rolloutBefore = await snapshotRollouts');
  assert.ok(diagnosticDispatch >= 0 && diagnosticDispatch < legacyPrivateCall);
});

test('public diagnostic failure exposes only closed governed fields', () => {
  const error = Object.create({ answerFailureStage: 'native_event_grammar' });
  Object.defineProperties(error, {
    answerFailureStage: { enumerable: true, value: 'native_event_grammar' },
    nativeEventFailureBoundary: { enumerable: true, value: 'raw_item_predicate' },
    nativeEventFailureRawItemPredicate: { enumerable: true, value: 'shape' },
    message: { enumerable: true, value: 'SECRET /private/path' },
    cause: { enumerable: true, value: 'SECRET_CAUSE' },
  });
  assert.deepEqual(publicDiagnosticFailure(error), {
    stage: 'native_event_grammar', boundary: 'raw_item_predicate', subreason: null,
    correlationOperand: null, rawItemPredicate: 'shape',
  });
  const serialized = JSON.stringify(publicDiagnosticFailure(error));
  assert.equal(serialized.includes('SECRET'), false);
  assert.equal(serialized.includes('/private/path'), false);
});
