/**
 * Custom Application Tracing Layer for Probe Chat
 * 
 * This module provides granular tracing that follows application logic closely,
 * replacing the generic Vercel AI SDK tracing with application-specific spans.
 */

import { trace, SpanStatusCode, SpanKind, context, TraceFlags } from '@opentelemetry/api';
import { randomUUID, createHash } from 'crypto';

/**
 * Convert a session ID to a valid OpenTelemetry trace ID (32-char hex)
 */
function sessionIdToTraceId(sessionId) {
  // Create a hash of the session ID and take first 32 chars
  const hash = createHash('sha256').update(sessionId).digest('hex');
  return hash.substring(0, 32);
}

// OpenTelemetry semantic conventions and custom attributes
const OTEL_ATTRS = {
  // Standard semantic conventions
  SERVICE_NAME: 'service.name',
  SERVICE_VERSION: 'service.version',
  HTTP_METHOD: 'http.method',
  HTTP_STATUS_CODE: 'http.status_code',
  ERROR_TYPE: 'error.type',
  ERROR_MESSAGE: 'error.message',
  
  // Custom application attributes following OpenTelemetry naming conventions
  APP_SESSION_ID: 'app.session.id',
  APP_MESSAGE_TYPE: 'app.message.type',
  APP_MESSAGE_CONTENT: 'app.message.content',
  APP_MESSAGE_LENGTH: 'app.message.length',
  APP_MESSAGE_HASH: 'app.message.hash',
  APP_AI_PROVIDER: 'app.ai.provider',
  APP_AI_MODEL: 'app.ai.model',
  APP_AI_TEMPERATURE: 'app.ai.temperature',
  APP_AI_MAX_TOKENS: 'app.ai.max_tokens',
  APP_AI_RESPONSE_CONTENT: 'app.ai.response.content',
  APP_AI_RESPONSE_LENGTH: 'app.ai.response.length',
  APP_AI_RESPONSE_HASH: 'app.ai.response.hash',
  APP_AI_COMPLETION_TOKENS: 'app.ai.completion_tokens',
  APP_AI_PROMPT_TOKENS: 'app.ai.prompt_tokens',
  APP_AI_FINISH_REASON: 'app.ai.finish_reason',
  APP_TOOL_NAME: 'app.tool.name',
  APP_TOOL_PARAMS: 'app.tool.params',
  APP_TOOL_RESULT: 'app.tool.result',
  APP_TOOL_SUCCESS: 'app.tool.success',
  APP_ITERATION_NUMBER: 'app.iteration.number'
};

class AppTracer {
  constructor() {
    // Use consistent tracer name across the application
    this.tracer = trace.getTracer('probe-chat', '1.0.0');
    this.activeSpans = new Map();
    this.sessionSpans = new Map();
    this.sessionContexts = new Map(); // Store active context for each session
  }

  /**
   * Get the shared tracer instance
   */
  getTracer() {
    return this.tracer;
  }

  /**
   * Hash a string for deduplication purposes
   */
  _hashString(str) {
    let hash = 0;
    if (str.length === 0) return hash;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash = hash & hash; // Convert to 32bit integer
    }
    return hash.toString();
  }

  /**
   * Get the active context for a session, creating spans within the session trace
   */
  _getSessionContext(sessionId) {
    return this.sessionContexts.get(sessionId) || context.active();
  }

  /**
   * Start a chat session span with custom trace ID based on session ID
   */
  startChatSession(sessionId, userMessage, provider, model) {
    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Starting chat session span for ${sessionId}`);
    }
    
    // Create a custom trace ID from the session ID
    const traceId = sessionIdToTraceId(sessionId);
    
    // Generate a span ID for the root span
    const spanId = randomUUID().replace(/-/g, '').substring(0, 16);
    
    // Create trace context with custom trace ID
    const spanContext = {
      traceId: traceId,
      spanId: spanId,
      traceFlags: TraceFlags.SAMPLED,
      isRemote: false
    };
    
    // Create a new context with our custom trace context
    const activeContext = trace.setSpanContext(context.active(), spanContext);
    
    // Start the span within this custom context
    const span = context.with(activeContext, () => {
      return this.tracer.startSpan('messaging.process', {
        kind: SpanKind.SERVER,
        attributes: {
          [OTEL_ATTRS.APP_SESSION_ID]: sessionId,
          [OTEL_ATTRS.APP_MESSAGE_CONTENT]: userMessage.substring(0, 500), // Capture more message content
          [OTEL_ATTRS.APP_MESSAGE_LENGTH]: userMessage.length,
          [OTEL_ATTRS.APP_MESSAGE_HASH]: this._hashString(userMessage), // Add hash for deduplication
          [OTEL_ATTRS.APP_AI_PROVIDER]: provider,
          [OTEL_ATTRS.APP_AI_MODEL]: model,
          'app.session.start_time': Date.now(),
          'app.trace.custom_id': true // Mark that we're using custom trace ID
        }
      });
    });

    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Created chat session span ${span.spanContext().spanId} in trace ${span.spanContext().traceId}`);
    }

    // Create session context with the span as the active span
    const sessionContext = trace.setSpan(context.active(), span);
    this.sessionContexts.set(sessionId, sessionContext);
    this.sessionSpans.set(sessionId, span);
    
    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Session context established for ${sessionId}`);
    }
    
    return span;
  }

  /**
   * Execute a function within the session context to ensure proper trace correlation
   */
  withSessionContext(sessionId, fn) {
    const sessionContext = this._getSessionContext(sessionId);
    return context.with(sessionContext, fn);
  }

  /**
   * Get the trace ID for a session (derived from session ID)
   */
  getTraceIdForSession(sessionId) {
    return sessionIdToTraceId(sessionId);
  }

  /**
   * Start processing a user message
   */
  startUserMessageProcessing(sessionId, messageId, message, imageUrlsFound = 0) {
    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Starting user message processing span for ${sessionId}`);
    }
    
    const sessionContext = this._getSessionContext(sessionId);
    
    return context.with(sessionContext, () => {
      // Get the parent span (should be the session span) from the context
      const parentSpan = trace.getActiveSpan();
      const spanOptions = {
        kind: SpanKind.INTERNAL,
        attributes: {
          [OTEL_ATTRS.APP_SESSION_ID]: sessionId,
          'app.message.id': messageId,
          [OTEL_ATTRS.APP_MESSAGE_TYPE]: 'user',
          [OTEL_ATTRS.APP_MESSAGE_CONTENT]: message.substring(0, 1000), // Include actual message content
          [OTEL_ATTRS.APP_MESSAGE_LENGTH]: message.length,
          [OTEL_ATTRS.APP_MESSAGE_HASH]: this._hashString(message),
          'app.message.image_urls_found': imageUrlsFound,
          'app.processing.start_time': Date.now()
        }
      };
      
      // Explicitly set the parent if available
      if (parentSpan) {
        spanOptions.parent = parentSpan.spanContext();
      }
      
      const span = this.tracer.startSpan('messaging.message.process', spanOptions);
      
      if (process.env.DEBUG_CHAT === '1') {
        console.log(`[DEBUG] AppTracer: Created user message processing span ${span.spanContext().spanId} with parent ${parentSpan?.spanContext().spanId}`);
      }
      
      this.activeSpans.set(`${sessionId}_user_processing`, span);
      // DO NOT overwrite the session context - this breaks parent-child relationships
      // Instead, create a temporary context for this message processing without storing it
      const messageContext = trace.setSpan(sessionContext, span);
      // Store the message context temporarily for child operations, but keep session context intact
      this.sessionContexts.set(`${sessionId}_message_processing`, messageContext);
      return span;
    });
  }

  /**
   * Execute a function within the context of user message processing span
   */
  withUserProcessingContext(sessionId, fn) {
    const span = this.activeSpans.get(`${sessionId}_user_processing`);
    if (span) {
      return context.with(trace.setSpan(context.active(), span), fn);
    }
    return fn();
  }

  /**
   * Start the agent loop
   */
  startAgentLoop(sessionId, maxIterations) {
    const sessionContext = this._getSessionContext(sessionId);
    
    return context.with(sessionContext, () => {
      // Get the parent span from the context
      const parentSpan = trace.getActiveSpan();
      const spanOptions = {
        kind: SpanKind.INTERNAL,
        attributes: {
          'app.session.id': sessionId,
          'app.loop.max_iterations': maxIterations,
          'app.loop.start_time': Date.now()
        }
      };
      
      // Explicitly set the parent if available
      if (parentSpan) {
        spanOptions.parent = parentSpan.spanContext();
      }
      
      const span = this.tracer.startSpan('agent.loop.start', spanOptions);
      
      this.activeSpans.set(`${sessionId}_agent_loop`, span);
      // DO NOT overwrite the session context - store agent loop context separately
      const agentLoopContext = trace.setSpan(sessionContext, span);
      this.sessionContexts.set(`${sessionId}_agent_loop`, agentLoopContext);
      return span;
    });
  }

  /**
   * Execute a function within the context of agent loop span
   */
  withAgentLoopContext(sessionId, fn) {
    const span = this.activeSpans.get(`${sessionId}_agent_loop`);
    if (span) {
      return context.with(trace.setSpan(context.active(), span), fn);
    }
    return fn();
  }

  /**
   * Start a single iteration of the agent loop
   */
  startAgentIteration(sessionId, iterationNumber, messagesCount, contextTokens) {
    const sessionContext = this._getSessionContext(sessionId);
    
    return context.with(sessionContext, () => {
      const span = this.tracer.startSpan('agent.loop.iteration', {
        kind: SpanKind.INTERNAL,
        attributes: {
          'app.session.id': sessionId,
          'app.iteration.number': iterationNumber,
          'app.iteration.messages_count': messagesCount,
          'app.iteration.context_tokens': contextTokens,
          'app.iteration.start_time': Date.now()
        }
      });
      
      this.activeSpans.set(`${sessionId}_iteration_${iterationNumber}`, span);
      // DO NOT overwrite the session context - store iteration context separately
      const iterationContext = trace.setSpan(sessionContext, span);
      this.sessionContexts.set(`${sessionId}_iteration_${iterationNumber}`, iterationContext);
      return span;
    });
  }

  /**
   * Execute a function within the context of agent iteration span
   */
  withIterationContext(sessionId, iterationNumber, fn) {
    const span = this.activeSpans.get(`${sessionId}_iteration_${iterationNumber}`);
    if (span) {
      return context.with(trace.setSpan(context.active(), span), fn);
    }
    return fn();
  }

  /**
   * Start an AI generation request
   */
  startAiGenerationRequest(sessionId, iterationNumber, model, provider, settings = {}, messagesContext = []) {
    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Starting AI generation request span for session ${sessionId}, iteration ${iterationNumber}`);
    }
    
    // Get the most appropriate context - prefer iteration context over session context
    const iterationContext = this.sessionContexts.get(`${sessionId}_iteration_${iterationNumber}`);
    const sessionContext = iterationContext || this._getSessionContext(sessionId);
    
    return context.with(sessionContext, () => {
      const span = this.tracer.startSpan('ai.generation.request', {
        kind: SpanKind.CLIENT,
        attributes: {
          [OTEL_ATTRS.APP_SESSION_ID]: sessionId,
          [OTEL_ATTRS.APP_ITERATION_NUMBER]: iterationNumber,
          [OTEL_ATTRS.APP_AI_MODEL]: model,
          [OTEL_ATTRS.APP_AI_PROVIDER]: provider,
          [OTEL_ATTRS.APP_AI_TEMPERATURE]: settings.temperature || 0,
          [OTEL_ATTRS.APP_AI_MAX_TOKENS]: settings.maxTokens || 0,
          'app.ai.max_retries': settings.maxRetries || 0,
          'app.ai.messages_count': messagesContext.length,
          'app.ai.request_start_time': Date.now()
        }
      });
      
      if (process.env.DEBUG_CHAT === '1') {
        console.log(`[DEBUG] AppTracer: Created AI generation span ${span.spanContext().spanId}`);
      }
      
      this.activeSpans.set(`${sessionId}_ai_request_${iterationNumber}`, span);
      // Store AI request context separately, don't overwrite session context
      const aiRequestContext = trace.setSpan(sessionContext, span);
      this.sessionContexts.set(`${sessionId}_ai_request_${iterationNumber}`, aiRequestContext);
      return span;
    });
  }

  /**
   * Record AI response received
   */
  recordAiResponse(sessionId, iterationNumber, responseData) {
    const sessionContext = this._getSessionContext(sessionId);
    
    return context.with(sessionContext, () => {
      const span = this.tracer.startSpan('ai.generation.response', {
        kind: SpanKind.INTERNAL,
        attributes: {
          [OTEL_ATTRS.APP_SESSION_ID]: sessionId,
          [OTEL_ATTRS.APP_ITERATION_NUMBER]: iterationNumber,
          [OTEL_ATTRS.APP_AI_RESPONSE_CONTENT]: responseData.response ? responseData.response.substring(0, 2000) : '', // Include actual response content
          [OTEL_ATTRS.APP_AI_RESPONSE_LENGTH]: responseData.responseLength || (responseData.response ? responseData.response.length : 0),
          [OTEL_ATTRS.APP_AI_RESPONSE_HASH]: responseData.response ? this._hashString(responseData.response) : '',
          [OTEL_ATTRS.APP_AI_COMPLETION_TOKENS]: responseData.completionTokens || 0,
          [OTEL_ATTRS.APP_AI_PROMPT_TOKENS]: responseData.promptTokens || 0,
          [OTEL_ATTRS.APP_AI_FINISH_REASON]: responseData.finishReason || 'unknown',
          'app.ai.response.time_to_first_chunk_ms': responseData.timeToFirstChunk || 0,
          'app.ai.response.time_to_finish_ms': responseData.timeToFinish || 0,
          'app.ai.response.received_time': Date.now()
        }
      });
      
      // End the span immediately since this is just recording the response
      span.setStatus({ code: SpanStatusCode.OK });
      span.end();
      return span;
    });
  }

  /**
   * Record a parsed tool call
   */
  recordToolCallParsed(sessionId, iterationNumber, toolName, toolParams) {
    const aiRequestSpan = this.activeSpans.get(`${sessionId}_ai_request_${iterationNumber}`);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.tool.name': toolName,
        'app.tool.params': JSON.stringify(toolParams).substring(0, 500), // Truncate large params
        'app.tool.parsed_time': Date.now()
      }
    };

    if (aiRequestSpan) {
      spanOptions.parent = aiRequestSpan.spanContext();
    }

    const span = this.tracer.startSpan('tool.call.parse', spanOptions);
    
    // End immediately since this is just recording the parsing
    span.setStatus({ code: SpanStatusCode.OK });
    span.end();

    return span;
  }

  /**
   * Start tool execution
   */
  startToolExecution(sessionId, iterationNumber, toolName, toolParams) {
    // Get the most appropriate context - prefer AI request context over session context  
    const aiRequestContext = this.sessionContexts.get(`${sessionId}_ai_request_${iterationNumber}`);
    const sessionContext = aiRequestContext || this._getSessionContext(sessionId);
    
    return context.with(sessionContext, () => {
      const span = this.tracer.startSpan('tool.call', {
        kind: SpanKind.INTERNAL,
        attributes: {
          [OTEL_ATTRS.APP_SESSION_ID]: sessionId,
          [OTEL_ATTRS.APP_ITERATION_NUMBER]: iterationNumber,
          [OTEL_ATTRS.APP_TOOL_NAME]: toolName,
          [OTEL_ATTRS.APP_TOOL_PARAMS]: JSON.stringify(toolParams).substring(0, 1000), // Include actual tool parameters
          'app.tool.params.hash': this._hashString(JSON.stringify(toolParams)),
          'app.tool.execution_start_time': Date.now(),
          // Add specific attributes based on tool type
          ...(toolName === 'search' && toolParams.query ? { 'app.tool.search.query': toolParams.query } : {}),
          ...(toolName === 'extract' && toolParams.file_path ? { 'app.tool.extract.file_path': toolParams.file_path } : {}),
          ...(toolName === 'query' && toolParams.pattern ? { 'app.tool.query.pattern': toolParams.pattern } : {}),
        }
      });
      
      this.activeSpans.set(`${sessionId}_tool_execution_${iterationNumber}`, span);
      // Store tool execution context separately, don't overwrite session context
      const toolExecutionContext = trace.setSpan(sessionContext, span);
      this.sessionContexts.set(`${sessionId}_tool_execution_${iterationNumber}`, toolExecutionContext);
      return span;
    });
  }

  /**
   * End tool execution with results
   */
  endToolExecution(sessionId, iterationNumber, success, resultLength = 0, errorMessage = null, result = null) {
    const span = this.activeSpans.get(`${sessionId}_tool_execution_${iterationNumber}`);
    if (!span) return;

    const attributes = {
      [OTEL_ATTRS.APP_TOOL_SUCCESS]: success,
      'app.tool.result_length': resultLength,
      'app.tool.execution_end_time': Date.now(),
      ...(errorMessage ? { [OTEL_ATTRS.ERROR_MESSAGE]: errorMessage } : {}),
      ...(result ? { 
        [OTEL_ATTRS.APP_TOOL_RESULT]: typeof result === 'string' ? result.substring(0, 2000) : JSON.stringify(result).substring(0, 2000),
        'app.tool.result.hash': this._hashString(typeof result === 'string' ? result : JSON.stringify(result))
      } : {})
    };

    span.setAttributes(attributes);

    span.setStatus({
      code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR,
      message: errorMessage
    });

    span.end();
    this.activeSpans.delete(`${sessionId}_tool_execution_${iterationNumber}`);
  }

  /**
   * End an iteration
   */
  endIteration(sessionId, iterationNumber, success = true, completedAction = null) {
    const span = this.activeSpans.get(`${sessionId}_iteration_${iterationNumber}`);
    if (!span) return;

    span.setAttributes({
      'app.iteration.success': success,
      'app.iteration.end_time': Date.now(),
      ...(completedAction ? { 'app.iteration.completed_action': completedAction } : {})
    });

    span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();
    this.activeSpans.delete(`${sessionId}_iteration_${iterationNumber}`);
  }

  /**
   * End the agent loop
   */
  endAgentLoop(sessionId, totalIterations, success = true, completionReason = null) {
    const span = this.activeSpans.get(`${sessionId}_agent_loop`);
    if (!span) return;

    span.setAttributes({
      'app.loop.total_iterations': totalIterations,
      'app.loop.success': success,
      'app.loop.end_time': Date.now(),
      ...(completionReason ? { 'app.loop.completion_reason': completionReason } : {})
    });

    span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();
    this.activeSpans.delete(`${sessionId}_agent_loop`);
  }

  /**
   * End user message processing
   */
  endUserMessageProcessing(sessionId, success = true) {
    const span = this.activeSpans.get(`${sessionId}_user_processing`);
    if (!span) {
      if (process.env.DEBUG_CHAT === '1') {
        console.log(`[DEBUG] AppTracer: No user message processing span found for ${sessionId}`);
      }
      return;
    }

    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Ending user message processing span ${span.spanContext().spanId} for ${sessionId}`);
    }

    span.setAttributes({
      'app.processing.success': success,
      'app.processing.end_time': Date.now()
    });

    span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();
    
    this.activeSpans.delete(`${sessionId}_user_processing`);
    // Clean up the message processing context
    this.sessionContexts.delete(`${sessionId}_message_processing`);
  }

  /**
   * End the chat session
   */
  endChatSession(sessionId, success = true, totalTokensUsed = 0) {
    const span = this.sessionSpans.get(sessionId);
    if (!span) {
      if (process.env.DEBUG_CHAT === '1') {
        console.log(`[DEBUG] AppTracer: No chat session span found for ${sessionId}`);
      }
      return;
    }

    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Ending chat session span ${span.spanContext().spanId} for ${sessionId}`);
    }

    span.setAttributes({
      'app.session.success': success,
      'app.session.total_tokens_used': totalTokensUsed,
      'app.session.end_time': Date.now()
    });

    span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();
    
    this.sessionSpans.delete(sessionId);
    // Clean up the session context after ending the span
    this.sessionContexts.delete(sessionId);
  }

  /**
   * End AI request span
   */
  endAiRequest(sessionId, iterationNumber, success = true) {
    const span = this.activeSpans.get(`${sessionId}_ai_request_${iterationNumber}`);
    if (!span) {
      if (process.env.DEBUG_CHAT === '1') {
        console.log(`[DEBUG] AppTracer: No AI request span found for ${sessionId}_ai_request_${iterationNumber}`);
      }
      return;
    }

    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Ending AI request span ${span.spanContext().spanId} for ${sessionId}, iteration ${iterationNumber}`);
    }

    span.setAttributes({
      'app.ai.request_success': success,
      'app.ai.request_end_time': Date.now()
    });

    span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();
    this.activeSpans.delete(`${sessionId}_ai_request_${iterationNumber}`);
  }

  /**
   * Record a completion attempt
   */
  recordCompletionAttempt(sessionId, success = true, finalResult = null) {
    const sessionSpan = this.sessionSpans.get(sessionId);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.completion.success': success,
        'app.completion.result_length': finalResult ? finalResult.length : 0,
        'app.completion.attempt_time': Date.now()
      }
    };

    if (sessionSpan) {
      spanOptions.parent = sessionSpan.spanContext();
    }

    const span = this.tracer.startSpan('agent.completion.attempt', spanOptions);
    
    span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();

    return span;
  }

  /**
   * Start image URL processing
   */
  startImageProcessing(sessionId, messageId, imageUrls = [], cleanedMessageLength = 0) {
    const userProcessingSpan = this.activeSpans.get(`${sessionId}_user_processing`);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.message.id': messageId,
        'app.image.urls_found': imageUrls.length,
        'app.image.message_cleaned_length': cleanedMessageLength,
        'app.image.processing_start_time': Date.now(),
        'app.image.urls_list': JSON.stringify(imageUrls).substring(0, 500)
      }
    };

    if (userProcessingSpan) {
      spanOptions.parent = userProcessingSpan.spanContext();
    }

    const span = this.tracer.startSpan('content.image.processing', spanOptions);
    this.activeSpans.set(`${sessionId}_image_processing`, span);
    return span;
  }

  /**
   * Record image URL validation results
   */
  recordImageValidation(sessionId, validationResults) {
    const imageProcessingSpan = this.activeSpans.get(`${sessionId}_image_processing`);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.image.validation.total_urls': validationResults.totalUrls || 0,
        'app.image.validation.valid_urls': validationResults.validUrls || 0,
        'app.image.validation.invalid_urls': validationResults.invalidUrls || 0,
        'app.image.validation.redirected_urls': validationResults.redirectedUrls || 0,
        'app.image.validation.timeout_urls': validationResults.timeoutUrls || 0,
        'app.image.validation.network_errors': validationResults.networkErrors || 0,
        'app.image.validation.duration_ms': validationResults.durationMs || 0,
        'app.image.validation_time': Date.now()
      }
    };

    if (imageProcessingSpan) {
      spanOptions.parent = imageProcessingSpan.spanContext();
    }

    const span = this.tracer.startSpan('content.image.validation', spanOptions);
    span.setStatus({ 
      code: validationResults.validUrls > 0 ? SpanStatusCode.OK : SpanStatusCode.ERROR,
      message: `${validationResults.validUrls}/${validationResults.totalUrls} URLs validated successfully`
    });
    span.end();
    return span;
  }

  /**
   * End image processing
   */
  endImageProcessing(sessionId, success = true, finalValidUrls = 0) {
    const span = this.activeSpans.get(`${sessionId}_image_processing`);
    if (!span) return;

    span.setAttributes({
      'app.image.processing_success': success,
      'app.image.final_valid_urls': finalValidUrls,
      'app.image.processing_end_time': Date.now()
    });

    span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();
    this.activeSpans.delete(`${sessionId}_image_processing`);
  }

  /**
   * Record AI model errors
   */
  recordAiModelError(sessionId, iterationNumber, errorDetails) {
    const aiRequestSpan = this.activeSpans.get(`${sessionId}_ai_request_${iterationNumber}`);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.error.type': 'ai_model_error',
        'app.error.category': errorDetails.category || 'unknown', // timeout, api_limit, network, etc.
        'app.error.message': errorDetails.message?.substring(0, 500) || '',
        'app.error.model': errorDetails.model || '',
        'app.error.provider': errorDetails.provider || '',
        'app.error.status_code': errorDetails.statusCode || 0,
        'app.error.retry_attempt': errorDetails.retryAttempt || 0,
        'app.error.timestamp': Date.now()
      }
    };

    if (aiRequestSpan) {
      spanOptions.parent = aiRequestSpan.spanContext();
    }

    const span = this.tracer.startSpan('ai.generation.error', spanOptions);
    span.setStatus({ code: SpanStatusCode.ERROR, message: errorDetails.message });
    span.end();
    return span;
  }

  /**
   * Record tool execution errors
   */
  recordToolError(sessionId, iterationNumber, toolName, errorDetails) {
    const toolExecutionSpan = this.activeSpans.get(`${sessionId}_tool_execution_${iterationNumber}`);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.error.type': 'tool_execution_error',
        'app.error.tool_name': toolName,
        'app.error.category': errorDetails.category || 'unknown', // validation, execution, network, filesystem
        'app.error.message': errorDetails.message?.substring(0, 500) || '',
        'app.error.exit_code': errorDetails.exitCode || 0,
        'app.error.signal': errorDetails.signal || '',
        'app.error.params': JSON.stringify(errorDetails.params || {}).substring(0, 300),
        'app.error.timestamp': Date.now()
      }
    };

    if (toolExecutionSpan) {
      spanOptions.parent = toolExecutionSpan.spanContext();
    }

    const span = this.tracer.startSpan('tool.call.error', spanOptions);
    span.setStatus({ code: SpanStatusCode.ERROR, message: errorDetails.message });
    span.end();
    return span;
  }

  /**
   * Record session cancellation
   */
  recordSessionCancellation(sessionId, reason = 'user_request', context = {}) {
    const sessionSpan = this.sessionSpans.get(sessionId);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.cancellation.reason': reason, // user_request, timeout, error, signal
        'app.cancellation.context': JSON.stringify(context).substring(0, 300),
        'app.cancellation.current_iteration': context.currentIteration || 0,
        'app.cancellation.active_tool': context.activeTool || '',
        'app.cancellation.timestamp': Date.now()
      }
    };

    if (sessionSpan) {
      spanOptions.parent = sessionSpan.spanContext();
    }

    const span = this.tracer.startSpan('messaging.session.cancel', spanOptions);
    span.setStatus({ code: SpanStatusCode.ERROR, message: `Session cancelled: ${reason}` });
    span.end();
    return span;
  }

  /**
   * Record token management metrics
   */
  recordTokenMetrics(sessionId, tokenData) {
    const sessionSpan = this.sessionSpans.get(sessionId);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.tokens.context_window': tokenData.contextWindow || 0,
        'app.tokens.current_total': tokenData.currentTotal || 0,
        'app.tokens.request_tokens': tokenData.requestTokens || 0,
        'app.tokens.response_tokens': tokenData.responseTokens || 0,
        'app.tokens.cache_read': tokenData.cacheRead || 0,
        'app.tokens.cache_write': tokenData.cacheWrite || 0,
        'app.tokens.utilization_percent': tokenData.contextWindow ? 
          Math.round((tokenData.currentTotal / tokenData.contextWindow) * 100) : 0,
        'app.tokens.measurement_time': Date.now()
      }
    };

    if (sessionSpan) {
      spanOptions.parent = sessionSpan.spanContext();
    }

    const span = this.tracer.startSpan('ai.token.metrics', spanOptions);
    span.setStatus({ code: SpanStatusCode.OK });
    span.end();
    return span;
  }

  /**
   * Record history management operations
   */
  recordHistoryOperation(sessionId, operation, details = {}) {
    const sessionSpan = this.sessionSpans.get(sessionId);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.history.operation': operation, // trim, update, clear, save
        'app.history.messages_before': details.messagesBefore || 0,
        'app.history.messages_after': details.messagesAfter || 0,
        'app.history.messages_removed': details.messagesRemoved || 0,
        'app.history.reason': details.reason || '', // max_length, memory_limit, session_reset
        'app.history.operation_time': Date.now()
      }
    };

    if (sessionSpan) {
      spanOptions.parent = sessionSpan.spanContext();
    }

    const span = this.tracer.startSpan('messaging.history.manage', spanOptions);
    span.setStatus({ code: SpanStatusCode.OK });
    span.end();
    return span;
  }

  /**
   * Record system prompt generation metrics
   */
  recordSystemPromptGeneration(sessionId, promptData) {
    const sessionSpan = this.sessionSpans.get(sessionId);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.prompt.base_length': promptData.baseLength || 0,
        'app.prompt.final_length': promptData.finalLength || 0,
        'app.prompt.files_added': promptData.filesAdded || 0,
        'app.prompt.generation_duration_ms': promptData.generationDurationMs || 0,
        'app.prompt.type': promptData.promptType || 'default',
        'app.prompt.estimated_tokens': promptData.estimatedTokens || 0,
        'app.prompt.generation_time': Date.now()
      }
    };

    if (sessionSpan) {
      spanOptions.parent = sessionSpan.spanContext();
    }

    const span = this.tracer.startSpan('ai.prompt.generate', spanOptions);
    span.setStatus({ code: SpanStatusCode.OK });
    span.end();
    return span;
  }

  /**
   * Record file system operations
   */
  recordFileSystemOperation(sessionId, operation, details = {}) {
    const activeSpan = this.activeSpans.get(`${sessionId}_tool_execution_${details.iterationNumber}`) || 
                      this.sessionSpans.get(sessionId);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.fs.operation': operation, // read, write, create_temp, delete, mkdir
        'app.fs.path': details.path?.substring(0, 200) || '',
        'app.fs.size_bytes': details.sizeBytes || 0,
        'app.fs.duration_ms': details.durationMs || 0,
        'app.fs.success': details.success !== false,
        'app.fs.error_code': details.errorCode || '',
        'app.fs.operation_time': Date.now()
      }
    };

    if (activeSpan) {
      spanOptions.parent = activeSpan.spanContext();
    }

    const span = this.tracer.startSpan('fs.operation', spanOptions);
    span.setStatus({ 
      code: details.success !== false ? SpanStatusCode.OK : SpanStatusCode.ERROR,
      message: details.errorMessage
    });
    span.end();
    return span;
  }

  /**
   * Record a generic event (used by completionPrompt and other features)
   * This provides compatibility with SimpleAppTracer interface
   * Adds an event to the existing session span rather than creating a new span
   */
  // visor-disable: Events are added to existing spans per OpenTelemetry convention - recordEvent semantically means adding an event, not creating a span. Both SimpleAppTracer and AppTracer now consistently add events.
  recordEvent(name, attributes = {}) {
    const sessionId = attributes['session.id'] || 'unknown';
    const sessionSpan = this.sessionSpans.get(sessionId);

    if (sessionSpan) {
      // Add event to the existing session span
      sessionSpan.addEvent(name, {
        'app.event.timestamp': Date.now(),
        ...attributes
      });
    } else if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: recordEvent called but no session span for ${sessionId}`);
    }
  }

  /**
   * Clean up any remaining active spans for a session
   */
  cleanup(sessionId) {
    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Cleaning up session ${sessionId}`);
    }
    
    // End any remaining active spans
    const keysToDelete = [];
    for (const [key, span] of this.activeSpans.entries()) {
      if (key.includes(sessionId)) {
        if (process.env.DEBUG_CHAT === '1') {
          console.log(`[DEBUG] AppTracer: Cleaning up active span ${key}`);
        }
        span.setStatus({ code: SpanStatusCode.ERROR, message: 'Session cleanup' });
        span.end();
        keysToDelete.push(key);
      }
    }
    keysToDelete.forEach(key => this.activeSpans.delete(key));

    // Only clean up session span if it still exists (wasn't properly ended by endChatSession)
    const sessionSpan = this.sessionSpans.get(sessionId);
    if (sessionSpan) {
      if (process.env.DEBUG_CHAT === '1') {
        console.log(`[DEBUG] AppTracer: Cleaning up orphaned session span for ${sessionId}`);
      }
      sessionSpan.setStatus({ code: SpanStatusCode.ERROR, message: 'Session cleanup - orphaned span' });
      sessionSpan.end();
      this.sessionSpans.delete(sessionId);
    }

    // Clean up all session-related contexts (session, message processing, iterations, AI requests, tool executions)
    const contextKeysToDelete = [];
    for (const [key] of this.sessionContexts.entries()) {
      if (key.includes(sessionId)) {
        contextKeysToDelete.push(key);
      }
    }
    contextKeysToDelete.forEach(key => this.sessionContexts.delete(key));
    
    if (process.env.DEBUG_CHAT === '1') {
      console.log(`[DEBUG] AppTracer: Session cleanup completed for ${sessionId}, cleaned ${contextKeysToDelete.length} contexts`);
    }
  }
}

// Export a singleton instance
export const appTracer = new AppTracer();