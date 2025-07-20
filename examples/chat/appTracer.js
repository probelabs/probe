/**
 * Custom Application Tracing Layer for Probe Chat
 * 
 * This module provides granular tracing that follows application logic closely,
 * replacing the generic Vercel AI SDK tracing with application-specific spans.
 */

import { trace, SpanStatusCode, SpanKind } from '@opentelemetry/api';
import { randomUUID } from 'crypto';

class AppTracer {
  constructor() {
    this.tracer = trace.getTracer('probe-chat-app', '1.0.0');
    this.activeSpans = new Map();
    this.sessionSpans = new Map();
  }

  /**
   * Start a chat session span
   */
  startChatSession(sessionId, userMessage, provider, model) {
    const span = this.tracer.startSpan('chat_session_start', {
      kind: SpanKind.SERVER,
      attributes: {
        'app.session.id': sessionId,
        'app.user.message': userMessage.substring(0, 200), // Truncate long messages
        'app.user.message.length': userMessage.length,
        'app.ai.provider': provider,
        'app.ai.model': model,
        'app.session.start_time': Date.now()
      }
    });

    this.sessionSpans.set(sessionId, span);
    return span;
  }

  /**
   * Start processing a user message
   */
  startUserMessageProcessing(sessionId, messageId, message, imageUrlsFound = 0) {
    const sessionSpan = this.sessionSpans.get(sessionId);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.message.id': messageId,
        'app.message.type': 'user',
        'app.message.content.length': message.length,
        'app.message.image_urls_found': imageUrlsFound,
        'app.processing.start_time': Date.now()
      }
    };

    // Set parent context if session span exists
    if (sessionSpan) {
      spanOptions.parent = sessionSpan.spanContext();
    }

    const span = this.tracer.startSpan('user_message_processing', spanOptions);
    this.activeSpans.set(`${sessionId}_user_processing`, span);
    return span;
  }

  /**
   * Start the agent loop
   */
  startAgentLoop(sessionId, maxIterations) {
    const userProcessingSpan = this.activeSpans.get(`${sessionId}_user_processing`);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.loop.max_iterations': maxIterations,
        'app.loop.start_time': Date.now()
      }
    };

    if (userProcessingSpan) {
      spanOptions.parent = userProcessingSpan.spanContext();
    }

    const span = this.tracer.startSpan('agent_loop_start', spanOptions);
    this.activeSpans.set(`${sessionId}_agent_loop`, span);
    return span;
  }

  /**
   * Start a single iteration of the agent loop
   */
  startAgentIteration(sessionId, iterationNumber, messagesCount, contextTokens) {
    const agentLoopSpan = this.activeSpans.get(`${sessionId}_agent_loop`);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.iteration.number': iterationNumber,
        'app.iteration.messages_count': messagesCount,
        'app.iteration.context_tokens': contextTokens,
        'app.iteration.start_time': Date.now()
      }
    };

    if (agentLoopSpan) {
      spanOptions.parent = agentLoopSpan.spanContext();
    }

    const span = this.tracer.startSpan('agent_loop_iteration', spanOptions);
    this.activeSpans.set(`${sessionId}_iteration_${iterationNumber}`, span);
    return span;
  }

  /**
   * Start an AI generation request
   */
  startAiGenerationRequest(sessionId, iterationNumber, model, provider, settings = {}) {
    const iterationSpan = this.activeSpans.get(`${sessionId}_iteration_${iterationNumber}`);
    const spanOptions = {
      kind: SpanKind.CLIENT,
      attributes: {
        'app.session.id': sessionId,
        'app.ai.model': model,
        'app.ai.provider': provider,
        'app.ai.temperature': settings.temperature || 0,
        'app.ai.max_tokens': settings.maxTokens || 0,
        'app.ai.max_retries': settings.maxRetries || 0,
        'app.ai.request_start_time': Date.now()
      }
    };

    if (iterationSpan) {
      spanOptions.parent = iterationSpan.spanContext();
    }

    const span = this.tracer.startSpan('ai_generation_request', spanOptions);
    this.activeSpans.set(`${sessionId}_ai_request_${iterationNumber}`, span);
    return span;
  }

  /**
   * Record AI response received
   */
  recordAiResponse(sessionId, iterationNumber, responseData) {
    const aiRequestSpan = this.activeSpans.get(`${sessionId}_ai_request_${iterationNumber}`);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.ai.response.length': responseData.responseLength || 0,
        'app.ai.response.completion_tokens': responseData.completionTokens || 0,
        'app.ai.response.prompt_tokens': responseData.promptTokens || 0,
        'app.ai.response.finish_reason': responseData.finishReason || 'unknown',
        'app.ai.response.time_to_first_chunk_ms': responseData.timeToFirstChunk || 0,
        'app.ai.response.time_to_finish_ms': responseData.timeToFinish || 0,
        'app.ai.response.received_time': Date.now()
      }
    };

    if (aiRequestSpan) {
      spanOptions.parent = aiRequestSpan.spanContext();
    }

    const span = this.tracer.startSpan('ai_response_received', spanOptions);
    
    // End the span immediately since this is just recording the response
    span.setStatus({ code: SpanStatusCode.OK });
    span.end();

    return span;
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

    const span = this.tracer.startSpan('tool_call_parsed', spanOptions);
    
    // End immediately since this is just recording the parsing
    span.setStatus({ code: SpanStatusCode.OK });
    span.end();

    return span;
  }

  /**
   * Start tool execution
   */
  startToolExecution(sessionId, iterationNumber, toolName, toolParams) {
    const aiRequestSpan = this.activeSpans.get(`${sessionId}_ai_request_${iterationNumber}`);
    const spanOptions = {
      kind: SpanKind.INTERNAL,
      attributes: {
        'app.session.id': sessionId,
        'app.tool.name': toolName,
        'app.tool.execution_start_time': Date.now(),
        // Add specific attributes based on tool type
        ...(toolName === 'search' && toolParams.query ? { 'app.tool.search.query': toolParams.query } : {}),
        ...(toolName === 'extract' && toolParams.file_path ? { 'app.tool.extract.file_path': toolParams.file_path } : {}),
        ...(toolName === 'query' && toolParams.pattern ? { 'app.tool.query.pattern': toolParams.pattern } : {}),
      }
    };

    if (aiRequestSpan) {
      spanOptions.parent = aiRequestSpan.spanContext();
    }

    const span = this.tracer.startSpan('tool_execution', spanOptions);
    this.activeSpans.set(`${sessionId}_tool_execution_${iterationNumber}`, span);
    return span;
  }

  /**
   * End tool execution with results
   */
  endToolExecution(sessionId, iterationNumber, success, resultLength = 0, errorMessage = null) {
    const span = this.activeSpans.get(`${sessionId}_tool_execution_${iterationNumber}`);
    if (!span) return;

    span.setAttributes({
      'app.tool.success': success,
      'app.tool.result_length': resultLength,
      'app.tool.execution_end_time': Date.now(),
      ...(errorMessage ? { 'app.tool.error_message': errorMessage } : {})
    });

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
    if (!span) return;

    span.setAttributes({
      'app.processing.success': success,
      'app.processing.end_time': Date.now()
    });

    span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();
    this.activeSpans.delete(`${sessionId}_user_processing`);
  }

  /**
   * End the chat session
   */
  endChatSession(sessionId, success = true, totalTokensUsed = 0) {
    const span = this.sessionSpans.get(sessionId);
    if (!span) return;

    span.setAttributes({
      'app.session.success': success,
      'app.session.total_tokens_used': totalTokensUsed,
      'app.session.end_time': Date.now()
    });

    span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();
    this.sessionSpans.delete(sessionId);
  }

  /**
   * End AI request span
   */
  endAiRequest(sessionId, iterationNumber, success = true) {
    const span = this.activeSpans.get(`${sessionId}_ai_request_${iterationNumber}`);
    if (!span) return;

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

    const span = this.tracer.startSpan('completion_attempt', spanOptions);
    
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

    const span = this.tracer.startSpan('image_url_processing', spanOptions);
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

    const span = this.tracer.startSpan('image_url_validation', spanOptions);
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

    const span = this.tracer.startSpan('ai_model_error', spanOptions);
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

    const span = this.tracer.startSpan('tool_execution_error', spanOptions);
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

    const span = this.tracer.startSpan('session_cancellation', spanOptions);
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

    const span = this.tracer.startSpan('token_metrics', spanOptions);
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

    const span = this.tracer.startSpan('history_management', spanOptions);
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

    const span = this.tracer.startSpan('system_prompt_generation', spanOptions);
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

    const span = this.tracer.startSpan('filesystem_operation', spanOptions);
    span.setStatus({ 
      code: details.success !== false ? SpanStatusCode.OK : SpanStatusCode.ERROR,
      message: details.errorMessage
    });
    span.end();
    return span;
  }

  /**
   * Clean up any remaining active spans for a session
   */
  cleanup(sessionId) {
    // End any remaining active spans
    const keysToDelete = [];
    for (const [key, span] of this.activeSpans.entries()) {
      if (key.includes(sessionId)) {
        span.setStatus({ code: SpanStatusCode.ERROR, message: 'Session cleanup' });
        span.end();
        keysToDelete.push(key);
      }
    }
    keysToDelete.forEach(key => this.activeSpans.delete(key));

    // End session span if still active
    const sessionSpan = this.sessionSpans.get(sessionId);
    if (sessionSpan) {
      sessionSpan.setStatus({ code: SpanStatusCode.ERROR, message: 'Session cleanup' });
      sessionSpan.end();
      this.sessionSpans.delete(sessionId);
    }
  }
}

// Export a singleton instance
export const appTracer = new AppTracer();