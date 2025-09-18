// Tests for ACP Connection
import { jest } from '@jest/globals';
import { EventEmitter } from 'events';
import { ACPConnection } from './connection.js';
import { ErrorCode, RequestMethod } from './types.js';

// Mock streams
class MockStream extends EventEmitter {
  constructor() {
    super();
    this.encoding = null;
    this.writtenData = [];
  }
  
  setEncoding(encoding) {
    this.encoding = encoding;
  }
  
  write(data) {
    this.writtenData.push(data);
    return true;
  }
}

describe('ACPConnection', () => {
  let inputStream, outputStream, connection;
  
  beforeEach(() => {
    inputStream = new MockStream();
    outputStream = new MockStream();
    connection = new ACPConnection(inputStream, outputStream);
  });
  
  afterEach(() => {
    // Clean up fake timers if they were used
    if (jest.isMockFunction(setTimeout)) {
      jest.useRealTimers();
    }
    connection.close();
  });
  
  describe('initialization', () => {
    test('should initialize with correct default values', () => {
      expect(connection.isConnected).toBe(false);
      expect(connection.messageId).toBe(1);
      expect(connection.buffer).toBe('');
      expect(connection.pendingRequests.size).toBe(0);
    });
    
    test('should setup streams correctly', () => {
      connection.start();
      expect(connection.isConnected).toBe(true);
      expect(inputStream.encoding).toBe('utf8');
    });
  });
  
  describe('message handling', () => {
    beforeEach(() => {
      connection.start();
    });
    
    test('should parse valid JSON-RPC messages', (done) => {
      const message = {
        jsonrpc: '2.0',
        method: RequestMethod.INITIALIZE,
        id: 1,
        params: { protocolVersion: '1' }
      };
      
      connection.on('request', (receivedMessage) => {
        expect(receivedMessage).toEqual(message);
        done();
      });
      
      inputStream.emit('data', JSON.stringify(message) + '\n');
    });
    
    test('should handle invalid JSON gracefully', () => {
      inputStream.emit('data', 'invalid json\n');
      
      // Should send parse error
      expect(outputStream.writtenData).toHaveLength(1);
      const response = JSON.parse(outputStream.writtenData[0]);
      expect(response.error.code).toBe(ErrorCode.PARSE_ERROR);
    });
    
    test('should handle notifications', (done) => {
      const notification = {
        jsonrpc: '2.0',
        method: 'someNotification',
        params: { data: 'test' }
      };
      
      connection.on('notification', (receivedMessage) => {
        expect(receivedMessage).toEqual(notification);
        done();
      });
      
      inputStream.emit('data', JSON.stringify(notification) + '\n');
    });
    
    test('should validate message format', () => {
      const invalidMessage = {
        jsonrpc: '1.0', // Wrong version
        method: 'test'
      };
      
      inputStream.emit('data', JSON.stringify(invalidMessage) + '\n');
      
      // Should send invalid request error
      expect(outputStream.writtenData).toHaveLength(1);
      const response = JSON.parse(outputStream.writtenData[0]);
      expect(response.error.code).toBe(ErrorCode.INVALID_REQUEST);
    });
  });
  
  describe('sending messages', () => {
    beforeEach(() => {
      connection.start();
    });
    
    test('should send requests with auto-incrementing IDs', async () => {
      const params = { test: 'data' };
      
      // Don't await - we'll resolve it manually
      const requestPromise = connection.sendRequest('testMethod', params);
      
      // Check that request was sent
      expect(outputStream.writtenData).toHaveLength(1);
      const sentMessage = JSON.parse(outputStream.writtenData[0]);
      expect(sentMessage).toEqual({
        jsonrpc: '2.0',
        method: 'testMethod',
        params,
        id: 1
      });
      
      // Simulate response
      const response = {
        jsonrpc: '2.0',
        id: 1,
        result: { success: true }
      };
      
      inputStream.emit('data', JSON.stringify(response) + '\n');
      
      const result = await requestPromise;
      expect(result).toEqual({ success: true });
    });
    
    test('should send notifications without ID', () => {
      const params = { notification: 'data' };
      
      connection.sendNotification('testNotification', params);
      
      expect(outputStream.writtenData).toHaveLength(1);
      const sentMessage = JSON.parse(outputStream.writtenData[0]);
      expect(sentMessage).toEqual({
        jsonrpc: '2.0',
        method: 'testNotification',
        params
      });
      expect(sentMessage.id).toBeUndefined();
    });
    
    test('should send responses', () => {
      const result = { data: 'test' };
      
      connection.sendResponse(123, result);
      
      expect(outputStream.writtenData).toHaveLength(1);
      const sentMessage = JSON.parse(outputStream.writtenData[0]);
      expect(sentMessage).toEqual({
        jsonrpc: '2.0',
        id: 123,
        result
      });
    });
    
    test('should send errors', () => {
      connection.sendError(456, ErrorCode.INTERNAL_ERROR, 'Test error', { detail: 'test' });
      
      expect(outputStream.writtenData).toHaveLength(1);
      const sentMessage = JSON.parse(outputStream.writtenData[0]);
      expect(sentMessage).toEqual({
        jsonrpc: '2.0',
        id: 456,
        error: {
          code: ErrorCode.INTERNAL_ERROR,
          message: 'Test error',
          data: { detail: 'test' }
        }
      });
    });
  });
  
  describe('request/response handling', () => {
    beforeEach(() => {
      connection.start();
    });
    
    test('should handle response errors', async () => {
      const requestPromise = connection.sendRequest('testMethod');
      
      const errorResponse = {
        jsonrpc: '2.0',
        id: 1,
        error: {
          code: ErrorCode.METHOD_NOT_FOUND,
          message: 'Method not found'
        }
      };
      
      inputStream.emit('data', JSON.stringify(errorResponse) + '\n');
      
      await expect(requestPromise).rejects.toThrow('RPC Error -32601: Method not found');
    });
    
    test('should timeout requests', async () => {
      // Set up fake timers before creating the promise
      jest.useFakeTimers();
      
      const requestPromise = connection.sendRequest('testMethod');
      
      // Fast-forward time past the timeout
      jest.advanceTimersByTime(30000);
      
      await expect(requestPromise).rejects.toThrow('Request timeout');
      
      jest.useRealTimers();
    });
  });
  
  describe('connection management', () => {
    test('should handle disconnection', (done) => {
      connection.start();
      
      connection.on('disconnect', () => {
        expect(connection.isConnected).toBe(false);
        done();
      });
      
      inputStream.emit('end');
    });
    
    test('should reject pending requests on close', async () => {
      connection.start();
      const requestPromise = connection.sendRequest('testMethod');
      
      connection.close();
      
      await expect(requestPromise).rejects.toThrow('Connection closed');
    });
    
    test('should handle stream errors', (done) => {
      connection.start();
      
      connection.on('error', (error) => {
        expect(error.message).toBe('Test error');
        done();
      });
      
      inputStream.emit('error', new Error('Test error'));
    });
  });
  
  describe('buffering', () => {
    beforeEach(() => {
      connection.start();
    });
    
    test('should handle partial messages', (done) => {
      const message = {
        jsonrpc: '2.0',
        method: 'test',
        params: { data: 'test' }
      };
      
      connection.on('notification', (receivedMessage) => {
        expect(receivedMessage).toEqual(message);
        done();
      });
      
      const jsonString = JSON.stringify(message) + '\n';
      
      // Send message in chunks
      inputStream.emit('data', jsonString.substring(0, 10));
      inputStream.emit('data', jsonString.substring(10, 20));
      inputStream.emit('data', jsonString.substring(20));
    });
    
    test('should handle multiple messages in one chunk', () => {
      const messages = [
        { jsonrpc: '2.0', method: 'test1' },
        { jsonrpc: '2.0', method: 'test2' }
      ];
      
      const receivedMessages = [];
      connection.on('notification', (message) => {
        receivedMessages.push(message);
      });
      
      const chunk = messages.map(msg => JSON.stringify(msg)).join('\n') + '\n';
      inputStream.emit('data', chunk);
      
      expect(receivedMessages).toHaveLength(2);
      expect(receivedMessages[0].method).toBe('test1');
      expect(receivedMessages[1].method).toBe('test2');
    });
  });
});