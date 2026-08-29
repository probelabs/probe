const GOVERNED_ANSWER_FAILURE_STAGES = new Set([
  'native_event_grammar', 'provider_engine', 'schema_result_validation', 'unknown',
]);
const GOVERNED_NATIVE_EVENT_FAILURE_BOUNDARIES = new Set([
  'raw_item_predicate', 'live_envelope_session',
]);
const GOVERNED_NATIVE_EVENT_FAILURE_SUBREASONS = new Set([
  'session_sequence', 'envelope_shape', 'correlation', 'attestation',
]);
const GOVERNED_NATIVE_EVENT_FAILURE_CORRELATION_OPERANDS = new Set([
  'thread_id', 'response_id',
]);

export class GovernedAnswerFailure extends Error {
  constructor(stage, nativeEventFailureBoundary = null, nativeEventFailureSubreason = null,
    nativeEventFailureCorrelationOperand = null) {
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
    if (answerFailureStage === 'native_event_grammar' &&
      nativeEventFailureBoundary === 'live_envelope_session') {
      Object.defineProperty(this, 'nativeEventFailureSubreason', {
        value: GOVERNED_NATIVE_EVENT_FAILURE_SUBREASONS.has(nativeEventFailureSubreason)
          ? nativeEventFailureSubreason : null,
        enumerable: true,
      });
    }
    if (answerFailureStage === 'native_event_grammar' &&
      nativeEventFailureBoundary === 'live_envelope_session' &&
      nativeEventFailureSubreason === 'correlation') {
      Object.defineProperty(this, 'nativeEventFailureCorrelationOperand', {
        value: GOVERNED_NATIVE_EVENT_FAILURE_CORRELATION_OPERANDS.has(nativeEventFailureCorrelationOperand)
          ? nativeEventFailureCorrelationOperand : null,
        enumerable: true,
      });
    }
    Object.freeze(this);
  }
}

export function governedAnswerFailure(stage, nativeEventFailureBoundary = null, nativeEventFailureSubreason = null,
  nativeEventFailureCorrelationOperand = null) {
  return new GovernedAnswerFailure(stage, nativeEventFailureBoundary, nativeEventFailureSubreason,
    nativeEventFailureCorrelationOperand);
}

export function normalizeGovernedAnswerFailure(error, fallback = 'unknown', nativeEventFailureBoundary = null,
  nativeEventFailureSubreason = null, nativeEventFailureCorrelationOperand = null) {
  return error instanceof GovernedAnswerFailure ? error
    : governedAnswerFailure(fallback, nativeEventFailureBoundary, nativeEventFailureSubreason,
      nativeEventFailureCorrelationOperand);
}
