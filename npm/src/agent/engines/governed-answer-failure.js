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
const GOVERNED_NATIVE_EVENT_FAILURE_ATTESTATION_PREDICATES = new Set([
  'event_shape', 'jsonrpc', 'params_shape', 'response_id', 'meta_shape', 'session_shape',
  'session_identity', 'model', 'model_provider', 'approval_policy', 'approvals_reviewer',
  'reasoning_effort', 'rollout_path', 'cwd', 'permission_shape', 'session_type', 'permission_type',
  'network',
  'filesystem_shape', 'filesystem_type', 'entries', 'entry', 'access', 'path_shape',
  'path_type', 'value_shape', 'kind', 'native_tool_evidence', 'internal_contract',
]);
const GOVERNED_SCHEMA_RESULT_VALIDATION_SUBREASONS = new Set([
  'response_json', 'schema_definition', 'schema_mismatch', 'result_identity',
]);
const GOVERNED_SCHEMA_RESULT_VALIDATION_KEYWORDS = new Set([
  'required', 'additionalProperties', 'type', 'pattern', 'enum', 'minItems', 'maxItems',
  'multiple', 'unknown',
]);
const GOVERNED_ATTESTATION_ERROR_PREDICATES = new Map([
  ['Invalid event', 'event_shape'], ['Invalid event.method', 'event_shape'],
  ['Invalid event.jsonrpc', 'jsonrpc'], ['Invalid event.params', 'params_shape'],
  ['Invalid event.params.id', 'response_id'], ['Invalid event._meta', 'meta_shape'],
  ['Invalid requestId', 'meta_shape'], ['Invalid event.msg', 'session_shape'],
  ['Invalid session identity', 'session_identity'], ['Invalid msg.model', 'model'],
  ['Invalid msg.model_provider_id', 'model_provider'], ['Invalid msg.approval_policy', 'approval_policy'],
  ['Invalid msg.approvals_reviewer', 'approvals_reviewer'],
  ['Invalid msg.reasoning_effort', 'reasoning_effort'], ['Invalid rollout_path', 'rollout_path'],
  ['Invalid msg.cwd', 'cwd'], ['Invalid cwd', 'cwd'],
  ['Invalid permission_profile', 'permission_shape'], ['Invalid permission_profile.type', 'permission_type'],
  ['Invalid permission_profile.network', 'network'], ['Invalid file_system', 'filesystem_shape'],
  ['Invalid file_system.type', 'filesystem_type'], ['Invalid file_system.entries', 'entries'],
  ['Invalid file_system entry', 'entry'], ['Invalid file_system entry access', 'access'],
  ['Invalid permission path', 'path_shape'], ['Invalid permission path type', 'path_type'],
  ['Invalid permission path value', 'value_shape'], ['Invalid permission path kind', 'kind'],
  ['Invalid msg.type', 'session_type'],
  ['Invalid native tool evidence', 'native_tool_evidence'],
  ['Invalid native tool total', 'native_tool_evidence'],
  ['Invalid native tool aggregates', 'native_tool_evidence'],
  ['Invalid native tool aggregate', 'native_tool_evidence'],
  ['Invalid undeclared native tool evidence', 'native_tool_evidence'],
  ['Invalid native tool status', 'native_tool_evidence'],
  ['Invalid native tool count', 'native_tool_evidence'],
  ['Invalid attester input', 'internal_contract'], ['Invalid events', 'internal_contract'],
  ['Invalid canonical JSON value', 'internal_contract'], ['Invalid profile', 'internal_contract'],
  ['Invalid profile.version', 'internal_contract'], ['Invalid profile.profileId', 'internal_contract'],
  ['Invalid profile.engine', 'internal_contract'], ['Invalid profile.model', 'internal_contract'],
  ['Invalid profile.reasoningEffort', 'internal_contract'], ['Invalid profile.sandbox', 'internal_contract'],
  ['Invalid profile.approvalPolicy', 'internal_contract'], ['Invalid profile.fallback', 'internal_contract'],
  ['Invalid profile.retries', 'internal_contract'], ['Invalid profile.probeTools', 'internal_contract'],
  ['Invalid profile.probeTools[0]', 'internal_contract'], ['Invalid profile.probeTools[1]', 'internal_contract'],
  ['Invalid profile.probeTools[2]', 'internal_contract'], ['Invalid profile.probeMcpTools', 'internal_contract'],
  ['Invalid profile.probeMcpTools[0]', 'internal_contract'],
  ['Invalid profile.probeMcpTools[1]', 'internal_contract'],
  ['Invalid profile.probeMcpTools[2]', 'internal_contract'],
  ['Invalid profile.codexNativeTools', 'internal_contract'],
  ['Invalid profile.codexNativeTools[0]', 'internal_contract'],
  ['Invalid profile capability overlap', 'internal_contract'],
]);

function governedAttestationPredicate(error) {
  return error instanceof TypeError ? GOVERNED_ATTESTATION_ERROR_PREDICATES.get(error.message) ?? null : null;
}

export class GovernedAnswerFailure extends Error {
  constructor(stage, nativeEventFailureBoundary = null, nativeEventFailureSubreason = null,
    nativeEventFailureCorrelationOperand = null, nativeEventFailureAttestationPredicate = null,
    schemaResultValidationSubreason = null, schemaResultValidationKeyword = null) {
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
    if (answerFailureStage === 'native_event_grammar' &&
      nativeEventFailureBoundary === 'live_envelope_session' &&
      nativeEventFailureSubreason === 'attestation') {
      Object.defineProperty(this, 'nativeEventFailureAttestationPredicate', {
        value: GOVERNED_NATIVE_EVENT_FAILURE_ATTESTATION_PREDICATES.has(nativeEventFailureAttestationPredicate)
          ? nativeEventFailureAttestationPredicate : null,
        enumerable: true,
      });
    }
    if (answerFailureStage === 'schema_result_validation') {
      const sanitizedSubreason = GOVERNED_SCHEMA_RESULT_VALIDATION_SUBREASONS.has(schemaResultValidationSubreason)
        ? schemaResultValidationSubreason : null;
      Object.defineProperty(this, 'schemaResultValidationSubreason', {
        value: sanitizedSubreason,
        enumerable: true,
      });
      Object.defineProperty(this, 'schemaResultValidationKeyword', {
        value: sanitizedSubreason === 'schema_mismatch'
          ? GOVERNED_SCHEMA_RESULT_VALIDATION_KEYWORDS.has(schemaResultValidationKeyword)
            ? schemaResultValidationKeyword : 'unknown'
          : null,
        enumerable: true,
      });
    }
    Object.freeze(this);
  }
}

export function governedAnswerFailure(stage, nativeEventFailureBoundary = null, nativeEventFailureSubreason = null,
  nativeEventFailureCorrelationOperand = null, nativeEventFailureAttestationPredicate = null,
  schemaResultValidationSubreason = null, schemaResultValidationKeyword = null) {
  return new GovernedAnswerFailure(stage, nativeEventFailureBoundary, nativeEventFailureSubreason,
    nativeEventFailureCorrelationOperand, nativeEventFailureAttestationPredicate,
    schemaResultValidationSubreason, schemaResultValidationKeyword);
}

export function normalizeGovernedAnswerFailure(error, fallback = 'unknown', nativeEventFailureBoundary = null,
  nativeEventFailureSubreason = null, nativeEventFailureCorrelationOperand = null,
  schemaResultValidationSubreason = null, schemaResultValidationKeyword = null) {
  return error instanceof GovernedAnswerFailure ? error
    : governedAnswerFailure(fallback, nativeEventFailureBoundary, nativeEventFailureSubreason,
      nativeEventFailureCorrelationOperand,
      fallback === 'native_event_grammar' && nativeEventFailureBoundary === 'live_envelope_session' &&
        nativeEventFailureSubreason === 'attestation' ? governedAttestationPredicate(error) : null,
      schemaResultValidationSubreason, schemaResultValidationKeyword);
}
