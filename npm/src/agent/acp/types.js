// Agent Client Protocol (ACP) type definitions
// Based on the ACP specification from Zed Industries

/**
 * ACP Protocol Version
 */
export const ACP_PROTOCOL_VERSION = "1";

/**
 * JSON-RPC 2.0 message types
 */
export const MessageType = {
  REQUEST: 'request',
  RESPONSE: 'response',
  NOTIFICATION: 'notification'
};

/**
 * ACP Request method names
 */
export const RequestMethod = {
  INITIALIZE: 'initialize',
  NEW_SESSION: 'newSession',
  LOAD_SESSION: 'loadSession',
  SET_SESSION_MODE: 'setSessionMode',
  PROMPT: 'prompt',
  CANCEL: 'cancel',
  
  // Client requests (that the agent can make to the client)
  WRITE_TEXT_FILE: 'writeTextFile',
  READ_TEXT_FILE: 'readTextFile',
  REQUEST_PERMISSION: 'requestPermission',
  CREATE_TERMINAL: 'createTerminal'
};

/**
 * ACP Notification method names
 */
export const NotificationMethod = {
  SESSION_UPDATED: 'sessionUpdated',
  TOOL_CALL_PROGRESS: 'toolCallProgress',
  MESSAGE_CHUNK: 'messageChunk',
  PLAN_UPDATED: 'planUpdated',
  AVAILABLE_COMMANDS: 'availableCommands'
};

/**
 * Tool call status types
 */
export const ToolCallStatus = {
  PENDING: 'pending',
  IN_PROGRESS: 'in_progress', 
  COMPLETED: 'completed',
  FAILED: 'failed'
};

/**
 * Tool call kinds
 */
export const ToolCallKind = {
  READ: 'read',
  edit: 'edit',
  delete: 'delete',
  execute: 'execute',
  search: 'search',
  query: 'query',
  extract: 'extract'
};

/**
 * Content block types
 */
export const ContentType = {
  TEXT: 'text',
  IMAGE: 'image',
  AUDIO: 'audio',
  RESOURCE: 'resource',
  EMBEDDED_RESOURCE: 'embedded_resource'
};

/**
 * Error codes (following JSON-RPC 2.0 spec)
 */
export const ErrorCode = {
  PARSE_ERROR: -32700,
  INVALID_REQUEST: -32600,
  METHOD_NOT_FOUND: -32601,
  INVALID_PARAMS: -32602,
  INTERNAL_ERROR: -32603,
  
  // ACP-specific errors
  UNSUPPORTED_PROTOCOL_VERSION: -32001,
  SESSION_NOT_FOUND: -32002,
  PERMISSION_DENIED: -32003,
  TOOL_EXECUTION_FAILED: -32004
};

/**
 * Session modes
 */
export const SessionMode = {
  NORMAL: 'normal',
  PLANNING: 'planning'
};

/**
 * Create a JSON-RPC 2.0 message
 */
export function createMessage(method, params = null, id = null) {
  const message = {
    jsonrpc: '2.0',
    method
  };
  
  if (params !== null) {
    message.params = params;
  }
  
  if (id !== null) {
    message.id = id;
  }
  
  return message;
}

/**
 * Create a JSON-RPC 2.0 response
 */
export function createResponse(id, result = null, error = null) {
  const response = {
    jsonrpc: '2.0',
    id
  };
  
  if (error) {
    response.error = error;
  } else {
    response.result = result;
  }
  
  return response;
}

/**
 * Create an error object
 */
export function createError(code, message, data = null) {
  const error = {
    code,
    message
  };
  
  if (data !== null) {
    error.data = data;
  }
  
  return error;
}

/**
 * Create a content block
 */
export function createContentBlock(type, content, metadata = {}) {
  return {
    type,
    ...content,
    ...metadata
  };
}

/**
 * Create a text content block
 */
export function createTextContent(text) {
  return createContentBlock(ContentType.TEXT, { text });
}

/**
 * Create a tool call progress notification
 */
export function createToolCallProgress(toolCallId, status, result = null, error = null) {
  const progress = {
    toolCallId,
    status
  };
  
  if (result !== null) {
    progress.result = result;
  }
  
  if (error !== null) {
    progress.error = error;
  }
  
  return progress;
}

/**
 * Validate JSON-RPC 2.0 message format
 */
export function validateMessage(message) {
  if (!message || typeof message !== 'object') {
    return { valid: false, error: 'Message must be an object' };
  }
  
  if (message.jsonrpc !== '2.0') {
    return { valid: false, error: 'Invalid or missing jsonrpc version' };
  }
  
  // Request validation
  if (message.id !== undefined) {
    if (!message.method && !message.result && !message.error) {
      return { valid: false, error: 'Request must have method or response must have result/error' };
    }
  }
  
  return { valid: true };
}