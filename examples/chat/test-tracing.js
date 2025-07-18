#!/usr/bin/env node

import { TelemetryConfig } from './telemetry.js';
import { existsSync, unlinkSync } from 'fs';

/**
 * Simple test script to verify tracing functionality
 */

console.log('Testing OpenTelemetry tracing setup...\n');

// Test 1: File tracing
console.log('🔍 Test 1: File tracing');
const testFile = './test-traces.jsonl';

// Clean up previous test file
if (existsSync(testFile)) {
  unlinkSync(testFile);
}

const fileConfig = new TelemetryConfig({
  serviceName: 'probe-chat-test',
  serviceVersion: '1.0.0',
  enableFile: true,
  enableConsole: false,
  enableRemote: false,
  filePath: testFile,
});

fileConfig.initialize();

// Create a test span
const span = fileConfig.createSpan('test-operation', {
  'test.attribute': 'test-value',
  'session.id': 'test-session-123'
});

if (span) {
  console.log('✅ Span created successfully');
  span.addEvent('Test event', { 'event.data': 'test-data' });
  span.end();
  
  // Give it a moment to write to file
  setTimeout(() => {
    if (existsSync(testFile)) {
      console.log('✅ Trace file created successfully');
      console.log(`📄 Trace file location: ${testFile}`);
      
      // Clean up
      unlinkSync(testFile);
      console.log('🧹 Test file cleaned up');
    } else {
      console.log('❌ Trace file not created');
    }
    
    // Test 2: Console tracing
    console.log('\n🔍 Test 2: Console tracing');
    
    const consoleConfig = new TelemetryConfig({
      serviceName: 'probe-chat-test',
      serviceVersion: '1.0.0',
      enableFile: false,
      enableConsole: true,
      enableRemote: false,
    });
    
    consoleConfig.initialize();
    
    const consoleSpan = consoleConfig.createSpan('console-test-operation', {
      'console.test': 'true'
    });
    
    if (consoleSpan) {
      console.log('✅ Console span created successfully');
      consoleSpan.addEvent('Console test event');
      consoleSpan.end();
    } else {
      console.log('❌ Console span creation failed');
    }
    
    // Test 3: Disabled tracing
    console.log('\n🔍 Test 3: Disabled tracing');
    
    const disabledConfig = new TelemetryConfig({
      serviceName: 'probe-chat-test',
      serviceVersion: '1.0.0',
      enableFile: false,
      enableConsole: false,
      enableRemote: false,
    });
    
    disabledConfig.initialize();
    
    const disabledSpan = disabledConfig.createSpan('disabled-operation');
    
    if (disabledSpan === null) {
      console.log('✅ Disabled tracing correctly returns null span');
    } else {
      console.log('❌ Disabled tracing should return null span');
    }
    
    console.log('\n🎉 All tests completed!');
    console.log('\nTo test with actual AI calls, run:');
    console.log('node index.js --trace-file --trace-console --message "Hello world"');
    
    // Shutdown telemetry
    fileConfig.shutdown();
    consoleConfig.shutdown();
    disabledConfig.shutdown();
  }, 1000);
} else {
  console.log('❌ Span creation failed');
  fileConfig.shutdown();
}