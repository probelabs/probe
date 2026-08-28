const GOVERNED_ANSWER_FAILURE_STAGES = new Set([
  'native_event_grammar', 'provider_engine', 'schema_result_validation', 'unknown',
]);

export class GovernedAnswerFailure extends Error {
  constructor(stage) {
    super();
    delete this.stack;
    Object.defineProperty(this, 'name', { value: 'GovernedAnswerFailure' });
    Object.defineProperty(this, 'answerFailureStage', {
      value: GOVERNED_ANSWER_FAILURE_STAGES.has(stage) ? stage : 'unknown',
      enumerable: true,
    });
    Object.freeze(this);
  }
}

export function governedAnswerFailure(stage) {
  return new GovernedAnswerFailure(stage);
}

export function normalizeGovernedAnswerFailure(error, fallback = 'unknown') {
  return error instanceof GovernedAnswerFailure ? error : governedAnswerFailure(fallback);
}
