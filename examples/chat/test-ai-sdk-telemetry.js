#!/usr/bin/env node

import { TelemetryConfig } from './telemetry.js';
import { trace, context } from '@opentelemetry/api';
import { readFileSync, unlinkSync, existsSync } from 'fs';

/**
 * Test script to simulate AI SDK telemetry behavior and verify tracing works end-to-end
 */

console.log('🧪 Testing AI SDK Telemetry Integration...\n');

// Clean up any existing test files
const testTraceFile = './test-ai-sdk-traces.jsonl';
if (existsSync(testTraceFile)) {
  unlinkSync(testTraceFile);
}

// Initialize telemetry
const telemetryConfig = new TelemetryConfig({
  serviceName: 'probe-chat-ai-sdk-test',
  serviceVersion: '1.0.0',
  enableFile: true,
  enableConsole: true,
  enableRemote: false,
  filePath: testTraceFile,
});

telemetryConfig.initialize();

console.log('✅ Telemetry initialized');

// Simulate AI SDK behavior with telemetry
async function simulateAISDKCall() {
  const tracer = telemetryConfig.getTracer();
  
  if (!tracer) {
    console.log('❌ No tracer available');
    return;
  }

  console.log('🎯 Starting simulated AI SDK call...');

  // Create a parent span for the overall operation
  const parentSpan = tracer.startSpan('ai.generateText', {
    attributes: {
      'ai.model.id': 'claude-3-7-sonnet-20250219',
      'ai.model.provider': 'anthropic',
      'ai.operation.name': 'generateText',
      'ai.request.model': 'claude-3-7-sonnet-20250219',
      'ai.request.max_tokens': 4096,
      'ai.request.temperature': 0.3,
      'ai.telemetry.functionId': 'chat-test-session-123',
      'ai.telemetry.metadata.sessionId': 'test-session-123',
      'ai.telemetry.metadata.iteration': '1',
      'ai.telemetry.metadata.model': 'claude-3-7-sonnet-20250219',
      'ai.telemetry.metadata.apiType': 'anthropic',
      'ai.telemetry.metadata.allowEdit': 'false',
      'ai.telemetry.metadata.promptType': 'default'
    }
  });

  try {
    // Set the context with the parent span
    await context.with(trace.setSpan(context.active(), parentSpan), async () => {
      
      // Simulate request processing
      parentSpan.addEvent('ai.request.start', {
        'ai.request.messages': '[{"role":"user","content":"Hello world"}]',
        'ai.request.system': 'You are a helpful assistant'
      });
      
      // Simulate some processing time
      await new Promise(resolve => setTimeout(resolve, 100));
      
      // Simulate stream processing
      const streamSpan = tracer.startSpan('ai.stream.process', {
        attributes: {
          'ai.stream.type': 'text',
          'ai.stream.provider': 'anthropic'
        }
      });
      
      try {
        streamSpan.addEvent('ai.stream.start');
        
        // Simulate streaming chunks
        for (let i = 0; i < 5; i++) {
          streamSpan.addEvent('ai.stream.chunk', {
            'ai.stream.chunk.index': i,
            'ai.stream.chunk.content': `chunk_${i}`
          });
          await new Promise(resolve => setTimeout(resolve, 20));
        }
        
        streamSpan.addEvent('ai.stream.end');
        streamSpan.setStatus({ code: 1 }); // OK status
        
      } catch (error) {
        streamSpan.setStatus({
          code: 2, // ERROR status
          message: error.message
        });
        streamSpan.recordException(error);
      } finally {
        streamSpan.end();
      }
      
      // Simulate response completion
      parentSpan.addEvent('ai.response.complete', {
        'ai.response.text': 'Hello! How can I help you today?',
        'ai.response.finish_reason': 'stop',
        'ai.usage.prompt_tokens': 15,
        'ai.usage.completion_tokens': 8,
        'ai.usage.total_tokens': 23
      });
      
      parentSpan.setStatus({ code: 1 }); // OK status
      
    });
    
  } catch (error) {
    parentSpan.setStatus({
      code: 2, // ERROR status
      message: error.message
    });
    parentSpan.recordException(error);
  } finally {
    parentSpan.end();
  }
}

// Run the simulation
try {
  await simulateAISDKCall();
  console.log('✅ AI SDK call simulation completed');
  
  // Give it a moment to write to file
  await new Promise(resolve => setTimeout(resolve, 500));
  
  // Check if trace file was created and has content
  if (existsSync(testTraceFile)) {
    const traceData = readFileSync(testTraceFile, 'utf8');
    console.log(`✅ Trace file created with ${traceData.split('\n').filter(Boolean).length} trace entries`);
    
    // Parse and validate trace data
    const traceLines = traceData.split('\n').filter(Boolean);
    
    if (traceLines.length > 0) {
      console.log('\n📊 Trace Analysis:');
      
      traceLines.forEach((line, index) => {
        try {
          const traceEntry = JSON.parse(line);
          console.log(`\n🔍 Trace Entry ${index + 1}:`);
          console.log(`  - Name: ${traceEntry.name}`);
          console.log(`  - Trace ID: ${traceEntry.traceId}`);
          console.log(`  - Span ID: ${traceEntry.spanId}`);
          console.log(`  - Status: ${traceEntry.status?.code || 'unknown'}`);
          console.log(`  - Start Time: ${new Date(traceEntry.startTimeUnixNano / 1000000).toISOString()}`);
          console.log(`  - Duration: ${(traceEntry.endTimeUnixNano - traceEntry.startTimeUnixNano) / 1000000}ms`);
          console.log(`  - Attributes: ${Object.keys(traceEntry.attributes || {}).length} attributes`);
          console.log(`  - Events: ${traceEntry.events?.length || 0} events`);
          
          // Check for AI-specific attributes
          const aiAttributes = Object.keys(traceEntry.attributes || {}).filter(key => key.startsWith('ai.'));
          if (aiAttributes.length > 0) {
            console.log(`  - AI Attributes: ${aiAttributes.join(', ')}`);
          }
          
        } catch (error) {
          console.log(`  ❌ Error parsing trace entry ${index + 1}: ${error.message}`);
        }
      });
      
      console.log('\n✅ All trace data parsed successfully!');
      
      // Save a sample for inspection
      console.log('\n📝 Sample trace entry:');
      console.log(JSON.stringify(JSON.parse(traceLines[0]), null, 2));
      
    } else {
      console.log('❌ Trace file is empty');
    }
    
  } else {
    console.log('❌ Trace file was not created');
  }
  
} catch (error) {
  console.error('❌ Test failed:', error);
} finally {
  // Clean up
  await telemetryConfig.shutdown();
  console.log('🧹 Telemetry shutdown complete');
  
  // Clean up test file
  if (existsSync(testTraceFile)) {
    unlinkSync(testTraceFile);
    console.log('🧹 Test trace file cleaned up');
  }
}

console.log('\n🎉 AI SDK Telemetry Integration Test Complete!');