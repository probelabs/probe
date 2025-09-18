// Tests for ACP Types and Utilities
import {
  ACP_PROTOCOL_VERSION,
  MessageType,
  RequestMethod,
  NotificationMethod,
  ToolCallStatus,
  ToolCallKind,
  ContentType,
  ErrorCode,
  SessionMode,
  createMessage,
  createResponse,
  createError,
  createContentBlock,
  createTextContent,
  createToolCallProgress,
  validateMessage
} from './types.js';

describe('ACP Types', () => {
  describe('constants', () => {
    test('should have correct protocol version', () => {
      expect(ACP_PROTOCOL_VERSION).toBe("1");
    });
    
    test('should have message types', () => {
      expect(MessageType.REQUEST).toBe('request');
      expect(MessageType.RESPONSE).toBe('response');
      expect(MessageType.NOTIFICATION).toBe('notification');
    });
    
    test('should have request methods', () => {
      expect(RequestMethod.INITIALIZE).toBe('initialize');
      expect(RequestMethod.NEW_SESSION).toBe('newSession');
      expect(RequestMethod.PROMPT).toBe('prompt');
      expect(RequestMethod.CANCEL).toBe('cancel');
    });
    
    test('should have notification methods', () => {
      expect(NotificationMethod.SESSION_UPDATED).toBe('sessionUpdated');
      expect(NotificationMethod.TOOL_CALL_PROGRESS).toBe('toolCallProgress');
      expect(NotificationMethod.MESSAGE_CHUNK).toBe('messageChunk');
    });
    
    test('should have tool call statuses', () => {
      expect(ToolCallStatus.PENDING).toBe('pending');
      expect(ToolCallStatus.IN_PROGRESS).toBe('in_progress');
      expect(ToolCallStatus.COMPLETED).toBe('completed');
      expect(ToolCallStatus.FAILED).toBe('failed');
    });
    
    test('should have tool call kinds', () => {
      expect(ToolCallKind.READ).toBe('read');
      expect(ToolCallKind.edit).toBe('edit');
      expect(ToolCallKind.search).toBe('search');
      expect(ToolCallKind.query).toBe('query');
      expect(ToolCallKind.extract).toBe('extract');
    });
    
    test('should have content types', () => {
      expect(ContentType.TEXT).toBe('text');
      expect(ContentType.IMAGE).toBe('image');
      expect(ContentType.RESOURCE).toBe('resource');
    });
    
    test('should have error codes', () => {
      expect(ErrorCode.PARSE_ERROR).toBe(-32700);
      expect(ErrorCode.INVALID_REQUEST).toBe(-32600);
      expect(ErrorCode.METHOD_NOT_FOUND).toBe(-32601);
      expect(ErrorCode.INVALID_PARAMS).toBe(-32602);
      expect(ErrorCode.INTERNAL_ERROR).toBe(-32603);
    });
    
    test('should have session modes', () => {
      expect(SessionMode.NORMAL).toBe('normal');
      expect(SessionMode.PLANNING).toBe('planning');
    });
  });
  
  describe('createMessage', () => {
    test('should create basic message', () => {
      const message = createMessage('testMethod');
      
      expect(message).toEqual({
        jsonrpc: '2.0',
        method: 'testMethod'
      });
    });
    
    test('should create message with params', () => {
      const params = { test: 'value' };
      const message = createMessage('testMethod', params);
      
      expect(message).toEqual({
        jsonrpc: '2.0',
        method: 'testMethod',
        params
      });
    });
    
    test('should create message with ID', () => {
      const message = createMessage('testMethod', null, 123);
      
      expect(message).toEqual({
        jsonrpc: '2.0',
        method: 'testMethod',
        id: 123
      });
    });
    
    test('should create message with params and ID', () => {
      const params = { test: 'value' };
      const message = createMessage('testMethod', params, 456);
      
      expect(message).toEqual({
        jsonrpc: '2.0',
        method: 'testMethod',
        params,
        id: 456
      });
    });
  });
  
  describe('createResponse', () => {
    test('should create success response', () => {
      const result = { success: true };
      const response = createResponse(123, result);
      
      expect(response).toEqual({
        jsonrpc: '2.0',
        id: 123,
        result
      });
    });
    
    test('should create error response', () => {
      const error = { code: -32601, message: 'Method not found' };
      const response = createResponse(456, null, error);
      
      expect(response).toEqual({
        jsonrpc: '2.0',
        id: 456,
        error
      });
    });
    
    test('should create response with null result', () => {
      const response = createResponse(789);
      
      expect(response).toEqual({
        jsonrpc: '2.0',
        id: 789,
        result: null
      });
    });
  });
  
  describe('createError', () => {
    test('should create basic error', () => {
      const error = createError(ErrorCode.INTERNAL_ERROR, 'Something went wrong');
      
      expect(error).toEqual({
        code: ErrorCode.INTERNAL_ERROR,
        message: 'Something went wrong'
      });
    });
    
    test('should create error with data', () => {
      const data = { details: 'More info' };
      const error = createError(ErrorCode.INVALID_PARAMS, 'Invalid parameters', data);
      
      expect(error).toEqual({
        code: ErrorCode.INVALID_PARAMS,
        message: 'Invalid parameters',
        data
      });
    });
  });
  
  describe('createContentBlock', () => {
    test('should create basic content block', () => {
      const content = { text: 'Hello world' };
      const block = createContentBlock(ContentType.TEXT, content);
      
      expect(block).toEqual({
        type: ContentType.TEXT,
        text: 'Hello world'
      });
    });
    
    test('should create content block with metadata', () => {
      const content = { uri: 'file://test.txt' };
      const metadata = { size: 1024, encoding: 'utf-8' };
      const block = createContentBlock(ContentType.RESOURCE, content, metadata);
      
      expect(block).toEqual({
        type: ContentType.RESOURCE,
        uri: 'file://test.txt',
        size: 1024,
        encoding: 'utf-8'
      });
    });
  });
  
  describe('createTextContent', () => {
    test('should create text content block', () => {
      const block = createTextContent('Hello world');
      
      expect(block).toEqual({
        type: ContentType.TEXT,
        text: 'Hello world'
      });
    });
  });
  
  describe('createToolCallProgress', () => {
    test('should create basic progress', () => {
      const progress = createToolCallProgress('tool-123', ToolCallStatus.IN_PROGRESS);
      
      expect(progress).toEqual({
        toolCallId: 'tool-123',
        status: ToolCallStatus.IN_PROGRESS
      });
    });
    
    test('should create progress with result', () => {
      const result = { found: 5 };
      const progress = createToolCallProgress('tool-456', ToolCallStatus.COMPLETED, result);
      
      expect(progress).toEqual({
        toolCallId: 'tool-456',
        status: ToolCallStatus.COMPLETED,
        result
      });
    });
    
    test('should create progress with error', () => {
      const error = 'Tool failed';
      const progress = createToolCallProgress('tool-789', ToolCallStatus.FAILED, null, error);
      
      expect(progress).toEqual({
        toolCallId: 'tool-789',
        status: ToolCallStatus.FAILED,
        error
      });
    });
  });
  
  describe('validateMessage', () => {
    test('should validate correct request message', () => {
      const message = {
        jsonrpc: '2.0',
        method: 'test',
        id: 1
      };
      
      const result = validateMessage(message);
      expect(result.valid).toBe(true);
    });
    
    test('should validate correct notification message', () => {
      const message = {
        jsonrpc: '2.0',
        method: 'test'
      };
      
      const result = validateMessage(message);
      expect(result.valid).toBe(true);
    });
    
    test('should validate correct response message', () => {
      const message = {
        jsonrpc: '2.0',
        id: 1,
        result: { success: true }
      };
      
      const result = validateMessage(message);
      expect(result.valid).toBe(true);
    });
    
    test('should validate correct error response message', () => {
      const message = {
        jsonrpc: '2.0',
        id: 1,
        error: { code: -32601, message: 'Method not found' }
      };
      
      const result = validateMessage(message);
      expect(result.valid).toBe(true);
    });
    
    test('should reject null/undefined message', () => {
      expect(validateMessage(null).valid).toBe(false);
      expect(validateMessage(undefined).valid).toBe(false);
      expect(validateMessage(null).error).toBe('Message must be an object');
    });
    
    test('should reject non-object message', () => {
      const result = validateMessage('not an object');
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Message must be an object');
    });
    
    test('should reject missing jsonrpc', () => {
      const message = { method: 'test', id: 1 };
      const result = validateMessage(message);
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Invalid or missing jsonrpc version');
    });
    
    test('should reject wrong jsonrpc version', () => {
      const message = { jsonrpc: '1.0', method: 'test', id: 1 };
      const result = validateMessage(message);
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Invalid or missing jsonrpc version');
    });
    
    test('should reject invalid request/response structure', () => {
      const message = { jsonrpc: '2.0', id: 1 }; // No method, result, or error
      const result = validateMessage(message);
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Request must have method or response must have result/error');
    });
  });
});