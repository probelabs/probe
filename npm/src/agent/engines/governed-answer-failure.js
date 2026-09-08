const GOVERNED_ANSWER_FAILURE_STAGES = new Set([
  'native_event_grammar', 'provider_engine', 'schema_result_validation', 'internal_contract', 'unknown',
]);
const GOVERNED_PROVIDER_ENGINE_FAILURE_BOUNDARIES = new Set(['acquire', 'query', 'close']);
const GOVERNED_NATIVE_EVENT_FAILURE_BOUNDARIES = new Set([
  'raw_item_predicate', 'live_envelope_session',
]);
const GOVERNED_NATIVE_EVENT_FAILURE_RAW_ITEM_PREDICATES = new Set([
  'shape', 'type', 'id', 'duplicate', 'phase', 'content', 'passthrough',
  'tool_name_or_allow', 'status', 'input', 'call_output_pairing', 'event_limit', 'tool_event_limit', 'tool_call_limit',
  'message_content_array', 'message_content_empty', 'message_content_limit', 'message_content_kind',
  'message_content_text_type', 'message_content_text_limit', 'reasoning_summary_array',
  'reasoning_summary_nonempty', 'reasoning_encrypted_content_type', 'reasoning_encrypted_content_limit',
  'tool_output_array', 'tool_output_limit', 'tool_output_kind', 'tool_output_text_type',
  'tool_output_text_limit',
  'final_answer_cardinality',
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
  'invocation_attestation', 'native_capability_aggregate',
]);
const GOVERNED_SCHEMA_RESULT_VALIDATION_SUBREASONS = new Set([
  'response_json', 'schema_definition', 'schema_mismatch', 'result_identity',
]);
const GOVERNED_SCHEMA_RESULT_VALIDATION_KEYWORDS = new Set([
  'required', 'additionalProperties', 'type', 'pattern', 'enum', 'minItems', 'maxItems',
  'multiple', 'unknown',
]);
const GOVERNED_CODEX_EXEC_FAILURE_VERSION = 'probe.governed-codex-exec-failure/v1';
const GOVERNED_CODEX_EXEC_FAILURE_CODES = new Set([
  'ANSWER_CARDINALITY', 'CANCELLED', 'CANONICAL', 'CLEANUP', 'CONFIG', 'DUPLICATE', 'EVENT',
  'EVENT_CATEGORY', 'EVENT_LIMIT', 'EVENT_ORDER', 'INCOMPLETE', 'INCOMPLETE_ITEM', 'ITEM',
  'ITEM_ORDER', 'ITEM_STATUS', 'JSONL', 'MCP', 'MCP_EVIDENCE', 'ONE_QUERY', 'OUTPUT_OVERFLOW',
  'SETUP', 'SPAWN', 'TIMEOUT', 'TOOL_POLICY', 'USAGE', 'VERSION', 'EXIT',
].map(code => `GOVERNED_CODEX_EXEC_${code}`));
const GOVERNED_CODEX_EXEC_STDERR_DIGEST = /^sha256:[0-9a-f]{64}$/;
const GOVERNED_CODEX_EXEC_ITEM_PREDICATES = new Set([
  'item_keys', 'item_id', 'item_text', 'item_phase', 'item_summary', 'item_server',
  'item_command', 'item_aggregated_output', 'item_exit_code', 'item_changes', 'item_status', 'tool_id',
]);
const GOVERNED_CODEX_EXEC_ITEM_EVENT_TYPES = new Set(['item.started', 'item.completed']);
const GOVERNED_CODEX_EXEC_ITEM_TYPES = new Set(['agent_message', 'reasoning', 'mcp_tool_call', 'command_execution', 'file_change']);
const GOVERNED_CODEX_EXEC_FIELD_TYPES = new Set(['null', 'array', 'object', 'string', 'number', 'boolean']);
const GOVERNED_CODEX_EXEC_SAFE_FIELD_NAME = /^[A-Za-z_][A-Za-z0-9_.-]{0,63}$/;
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

function closeRejectedItemFields(value) {
  if (!Array.isArray(value) || value.length > 32) return null;
  let previousName = null;
  const fields = [];
  for (const field of value) {
    if (!field || typeof field !== 'object' || Array.isArray(field)) return null;
    const keys = Object.keys(field);
    if (keys.length < 2 || keys.length > 3 || !keys.includes('name') || !keys.includes('type') ||
        (keys.length === 3 && !keys.includes('size'))) return null;
    const name = ownDataValue(field, 'name');
    const type = ownDataValue(field, 'type');
    if (typeof name !== 'string' || (name !== '<unsafe>' && !GOVERNED_CODEX_EXEC_SAFE_FIELD_NAME.test(name)) ||
        !GOVERNED_CODEX_EXEC_FIELD_TYPES.has(type) || (previousName !== null && name < previousName)) return null;
    const hasSize = Object.prototype.hasOwnProperty.call(field, 'size');
    const size = ownDataValue(field, 'size');
    if (hasSize && (type !== 'string' && type !== 'array' || !Number.isSafeInteger(size) || size < 0)) return null;
    if (!hasSize && (type === 'string' || type === 'array')) return null;
    fields.push(Object.freeze({ name, type, ...(hasSize ? { size } : {}) }));
    previousName = name;
  }
  return Object.freeze(fields);
}

function closeRejectedItemEvent(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value) ||
      Object.keys(value).length !== 6 || Object.keys(value).some(key =>
        !['source', 'predicate', 'eventType', 'itemType', 'eventFields', 'itemFields'].includes(key))) return null;
  const source = ownDataValue(value, 'source');
  const predicate = ownDataValue(value, 'predicate');
  const eventType = ownDataValue(value, 'eventType');
  const itemType = ownDataValue(value, 'itemType');
  const eventFields = closeRejectedItemFields(ownDataValue(value, 'eventFields'));
  const itemFields = closeRejectedItemFields(ownDataValue(value, 'itemFields'));
  if (source !== 'codex-exec-rejected-item/v1' || !GOVERNED_CODEX_EXEC_ITEM_PREDICATES.has(predicate) ||
      !GOVERNED_CODEX_EXEC_ITEM_EVENT_TYPES.has(eventType) || !GOVERNED_CODEX_EXEC_ITEM_TYPES.has(itemType) ||
      !eventFields || !itemFields) return null;
  return Object.freeze({ source, predicate, eventType, itemType, eventFields, itemFields });
}

function ownDataValue(value, key) {
  if (!value || (typeof value !== 'object' && typeof value !== 'function')) return undefined;
  try {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    return descriptor && 'value' in descriptor ? descriptor.value : undefined;
  } catch { return undefined; }
}

function closeProviderEngineDiagnostic(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const keys = Object.keys(value);
  if (keys.length < 2 || keys.length > 4 || !keys.includes('version') || !keys.includes('code') ||
      keys.some(key => !['version', 'code', 'stderr', 'event'].includes(key))) return null;
  const version = ownDataValue(value, 'version');
  const code = ownDataValue(value, 'code');
  if (version !== GOVERNED_CODEX_EXEC_FAILURE_VERSION || typeof code !== 'string' || !GOVERNED_CODEX_EXEC_FAILURE_CODES.has(code)) return null;
  const stderr = ownDataValue(value, 'stderr');
  let closedStderr;
  if (stderr !== undefined) {
    if (!stderr || typeof stderr !== 'object' || Array.isArray(stderr) ||
      ![...Object.keys(stderr)].every(key => ['source', 'bytes', 'digest', 'safeMessage'].includes(key)) ||
      Object.keys(stderr).length < 3 || Object.keys(stderr).length > 4 ||
      ownDataValue(stderr, 'source') !== 'codex-exec-stderr/v1' ||
      !Number.isSafeInteger(ownDataValue(stderr, 'bytes')) || ownDataValue(stderr, 'bytes') < 1 || ownDataValue(stderr, 'bytes') > 1048576 ||
      !GOVERNED_CODEX_EXEC_STDERR_DIGEST.test(ownDataValue(stderr, 'digest'))) return null;
    const safeMessage = ownDataValue(stderr, 'safeMessage');
    closedStderr = Object.freeze({ source: 'codex-exec-stderr/v1', bytes: ownDataValue(stderr, 'bytes'), digest: ownDataValue(stderr, 'digest'),
      ...(safeMessage === 'access_token_refresh_revoked' ? { safeMessage } : {}) });
  }
  const event = ownDataValue(value, 'event');
  const closedEvent = event === undefined ? null : closeRejectedItemEvent(event);
  if (event !== undefined && !closedEvent) return null;
  return Object.freeze({ version, code, ...(closedStderr ? { stderr: closedStderr } : {}), ...(closedEvent ? { event: closedEvent } : {}) });
}

export class GovernedAnswerFailure extends Error {
  constructor(stage, nativeEventFailureBoundary = null, nativeEventFailureSubreason = null,
    nativeEventFailureCorrelationOperand = null, nativeEventFailureAttestationPredicate = null,
    schemaResultValidationSubreason = null, schemaResultValidationKeyword = null,
    providerEngineFailureBoundary = null, nativeEventFailureRawItemPredicate = null,
    providerEngineDiagnostic = null) {
    super();
    delete this.stack;
    const answerFailureStage = GOVERNED_ANSWER_FAILURE_STAGES.has(stage) ? stage : 'unknown';
    Object.defineProperty(this, 'name', { value: 'GovernedAnswerFailure' });
    Object.defineProperty(this, 'answerFailureStage', {
      value: answerFailureStage,
      enumerable: true,
    });
    if (answerFailureStage === 'provider_engine') Object.defineProperty(this, 'providerEngineFailureBoundary', {
      value: GOVERNED_PROVIDER_ENGINE_FAILURE_BOUNDARIES.has(providerEngineFailureBoundary)
        ? providerEngineFailureBoundary : null,
      enumerable: true,
    });
    if (answerFailureStage === 'provider_engine' && providerEngineDiagnostic !== null &&
      providerEngineDiagnostic !== undefined) {
      const closedDiagnostic = closeProviderEngineDiagnostic(providerEngineDiagnostic);
      if (closedDiagnostic) Object.defineProperty(this, 'providerEngineDiagnostic', {
        value: closedDiagnostic,
        enumerable: true,
      });
    }
    if (answerFailureStage === 'native_event_grammar') Object.defineProperty(this, 'nativeEventFailureBoundary', {
      value: GOVERNED_NATIVE_EVENT_FAILURE_BOUNDARIES.has(nativeEventFailureBoundary)
        ? nativeEventFailureBoundary : null,
      enumerable: true,
    });
    if (answerFailureStage === 'native_event_grammar' &&
      nativeEventFailureBoundary === 'raw_item_predicate') {
      Object.defineProperty(this, 'nativeEventFailureRawItemPredicate', {
        value: GOVERNED_NATIVE_EVENT_FAILURE_RAW_ITEM_PREDICATES.has(nativeEventFailureRawItemPredicate)
          ? nativeEventFailureRawItemPredicate : null,
        enumerable: true,
      });
    }
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
  schemaResultValidationSubreason = null, schemaResultValidationKeyword = null,
  providerEngineFailureBoundary = null, nativeEventFailureRawItemPredicate = null,
  providerEngineDiagnostic = null) {
  return new GovernedAnswerFailure(stage, nativeEventFailureBoundary, nativeEventFailureSubreason,
    nativeEventFailureCorrelationOperand, nativeEventFailureAttestationPredicate,
    schemaResultValidationSubreason, schemaResultValidationKeyword, providerEngineFailureBoundary,
    nativeEventFailureRawItemPredicate, providerEngineDiagnostic);
}

export function normalizeGovernedAnswerFailure(error, fallback = 'unknown', nativeEventFailureBoundary = null,
  nativeEventFailureSubreason = null, nativeEventFailureCorrelationOperand = null,
  schemaResultValidationSubreason = null, schemaResultValidationKeyword = null,
  providerEngineFailureBoundary = null, nativeEventFailureAttestationPredicate = null,
  nativeEventFailureRawItemPredicate = null) {
  return error instanceof GovernedAnswerFailure ? error
    : governedAnswerFailure(fallback, nativeEventFailureBoundary, nativeEventFailureSubreason,
      nativeEventFailureCorrelationOperand,
      fallback === 'native_event_grammar' && nativeEventFailureBoundary === 'live_envelope_session' &&
        nativeEventFailureSubreason === 'attestation'
        ? nativeEventFailureAttestationPredicate ?? governedAttestationPredicate(error) : null,
      schemaResultValidationSubreason, schemaResultValidationKeyword, providerEngineFailureBoundary,
      fallback === 'native_event_grammar' && nativeEventFailureBoundary === 'raw_item_predicate'
        ? nativeEventFailureRawItemPredicate : null);
}
