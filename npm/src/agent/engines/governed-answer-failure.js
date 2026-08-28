const GOVERNED_ANSWER_FAILURE_STAGES = new Set([
  'native_event_grammar', 'provider_engine', 'schema_result_validation', 'unknown',
]);
const GOVERNED_NATIVE_EVENT_FAILURE_BOUNDARIES = new Set([
  'raw_item_predicate', 'live_envelope_session',
]);

export class GovernedAnswerFailure extends Error {
  constructor(stage, nativeEventFailureBoundary = null) {
    super();
    delete this.stack;
    const answerFailureStage = GOVERNED_ANSWER_FAILURE_STAGES.has(stage) ? stage : 'unknown';
    Object.defineProperty(this, 'name', { value: 'GovernedAnswerFailure' });
    Object.defineProperty(this, 'answerFailureStage', {
      value: answerFailureStage,
      enumerable: true,
    });
    if (answerFailureStage === 'native_event_grammar') Object.defineProperty(this, 'nativeEventFailureBoundary', {
      value: GOVERNED_NATIVE_EVENT_FAILURE_BOUNDARIES.has(nativeEventFailureBoundary)
        ? nativeEventFailureBoundary : null,
      enumerable: true,
    });
    Object.freeze(this);
  }
}

export function governedAnswerFailure(stage, nativeEventFailureBoundary = null) {
  return new GovernedAnswerFailure(stage, nativeEventFailureBoundary);
}

export function normalizeGovernedAnswerFailure(error, fallback = 'unknown', nativeEventFailureBoundary = null) {
  return error instanceof GovernedAnswerFailure ? error : governedAnswerFailure(fallback, nativeEventFailureBoundary);
}
