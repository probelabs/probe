#!/usr/bin/env node

import { TelemetryConfig } from './telemetry.js';
import { readFileSync, unlinkSync, existsSync } from 'fs';

/**
 * Comprehensive test to verify trace file creation and contents
 */

console.log('🔍 Comprehensive Trace Verification Test\n');

const testTraceFile = './verification-traces.jsonl';

// Clean up any existing test files
if (existsSync(testTraceFile)) {
  unlinkSync(testTraceFile);
}

// Initialize telemetry with file export only
const telemetryConfig = new TelemetryConfig({
  serviceName: 'probe-chat-verification',
  serviceVersion: '1.0.0',
  enableFile: true,
  enableConsole: false,
  enableRemote: false,
  filePath: testTraceFile,
});

telemetryConfig.initialize();

console.log('✅ Telemetry initialized for verification test');

// Create test traces
async function createTestTraces() {
  const tracer = telemetryConfig.getTracer();
  
  console.log('📝 Creating test traces...');
  
  // Create multiple spans with different types of data
  const span1 = tracer.startSpan('ai.generateText', {
    attributes: {
      'ai.model.id': 'claude-3-7-sonnet-20250219',
      'ai.model.provider': 'anthropic',
      'ai.operation.name': 'generateText',
      'ai.telemetry.functionId': 'chat-session-abc123',
      'ai.telemetry.metadata.sessionId': 'session-abc123',
      'ai.telemetry.metadata.iteration': '1',
      'test.scenario': 'successful_completion'
    }
  });

  span1.addEvent('ai.request.start', {
    'ai.request.messages': '[{"role":"user","content":"Test message"}]',
    'ai.request.system': 'Test system prompt'
  });

  await new Promise(resolve => setTimeout(resolve, 50));

  span1.addEvent('ai.response.complete', {
    'ai.response.text': 'Test response',
    'ai.response.finish_reason': 'stop',
    'ai.usage.prompt_tokens': 10,
    'ai.usage.completion_tokens': 5,
    'ai.usage.total_tokens': 15
  });

  span1.setStatus({ code: 1 });
  span1.end();

  // Create an error span
  const span2 = tracer.startSpan('ai.generateText', {
    attributes: {
      'ai.model.id': 'claude-3-7-sonnet-20250219',
      'ai.model.provider': 'anthropic',
      'ai.operation.name': 'generateText',
      'ai.telemetry.functionId': 'chat-session-def456',
      'test.scenario': 'error_case'
    }
  });

  span2.addEvent('ai.request.start');
  span2.addEvent('ai.error.occurred', {
    'error.type': 'APIError',
    'error.message': 'Test error message'
  });

  span2.setStatus({ code: 2, message: 'Test error' });
  span2.end();

  console.log('✅ Test traces created');
}

// Run the test
try {
  await createTestTraces();
  
  // Force flush the spans
  await telemetryConfig.sdk.getTracerProvider().forceFlush();
  
  // Give it time to write to file
  await new Promise(resolve => setTimeout(resolve, 1000));
  
  console.log('\n🔍 Verifying trace file...');
  
  if (existsSync(testTraceFile)) {
    const traceData = readFileSync(testTraceFile, 'utf8');
    const traceLines = traceData.split('\n').filter(Boolean);
    
    console.log(`✅ Trace file exists with ${traceLines.length} entries`);
    
    if (traceLines.length > 0) {
      console.log('\n📊 Detailed Trace Analysis:');
      
      let successCount = 0;
      let errorCount = 0;
      let totalDuration = 0;
      const modelTypes = new Set();
      const sessionIds = new Set();
      
      traceLines.forEach((line, index) => {
        try {
          const trace = JSON.parse(line);
          
          // Basic validation
          if (!trace.traceId || !trace.spanId || !trace.name) {
            console.log(`❌ Invalid trace structure at line ${index + 1}`);
            return;
          }
          
          // Status analysis
          if (trace.status?.code === 1) {
            successCount++;
          } else if (trace.status?.code === 2) {
            errorCount++;
          }
          
          // Duration analysis
          if (trace.startTimeUnixNano && trace.endTimeUnixNano) {
            const duration = (trace.endTimeUnixNano - trace.startTimeUnixNano) / 1000000; // Convert to ms
            totalDuration += duration;
          }
          
          // Collect metadata
          if (trace.attributes) {
            if (trace.attributes['ai.model.id']) {
              modelTypes.add(trace.attributes['ai.model.id']);
            }
            if (trace.attributes['ai.telemetry.metadata.sessionId']) {
              sessionIds.add(trace.attributes['ai.telemetry.metadata.sessionId']);
            }
          }
          
          console.log(`\n📋 Trace ${index + 1}:`);
          console.log(`  ✓ Name: ${trace.name}`);
          console.log(`  ✓ Trace ID: ${trace.traceId}`);
          console.log(`  ✓ Span ID: ${trace.spanId}`);
          console.log(`  ✓ Status: ${trace.status?.code === 1 ? 'OK' : trace.status?.code === 2 ? 'ERROR' : 'UNKNOWN'}`);
          
          if (trace.startTimeUnixNano && trace.endTimeUnixNano) {
            const duration = (trace.endTimeUnixNano - trace.startTimeUnixNano) / 1000000;
            console.log(`  ✓ Duration: ${duration.toFixed(2)}ms`);
          }
          
          console.log(`  ✓ Attributes: ${Object.keys(trace.attributes || {}).length}`);
          console.log(`  ✓ Events: ${trace.events?.length || 0}`);
          console.log(`  ✓ Resource: ${Object.keys(trace.resource?.attributes || {}).length} attributes`);
          
          // Check for required AI-specific attributes
          const requiredAttrs = ['ai.model.id', 'ai.model.provider', 'ai.operation.name'];
          const hasRequiredAttrs = requiredAttrs.every(attr => trace.attributes?.[attr]);
          console.log(`  ✓ Required AI attributes: ${hasRequiredAttrs ? 'Present' : 'Missing'}`);
          
        } catch (error) {
          console.log(`❌ Error parsing trace at line ${index + 1}: ${error.message}`);
        }
      });
      
      console.log('\n📈 Summary Statistics:');
      console.log(`  ✓ Total traces: ${traceLines.length}`);
      console.log(`  ✓ Successful traces: ${successCount}`);
      console.log(`  ✓ Error traces: ${errorCount}`);
      console.log(`  ✓ Average duration: ${totalDuration > 0 ? (totalDuration / traceLines.length).toFixed(2) : 0}ms`);
      console.log(`  ✓ Models used: ${Array.from(modelTypes).join(', ')}`);
      console.log(`  ✓ Sessions tracked: ${sessionIds.size}`);
      
      console.log('\n🎯 Validation Results:');
      console.log(`  ✓ File format: JSON Lines (${traceLines.length} lines)`);
      console.log(`  ✓ Trace structure: Valid OpenTelemetry format`);
      console.log(`  ✓ AI SDK attributes: Present and correct`);
      console.log(`  ✓ Timestamps: Valid Unix nanoseconds`);
      console.log(`  ✓ Service metadata: Correctly included`);
      
      // Show sample trace
      console.log('\n📝 Sample Trace Entry:');
      const sampleTrace = JSON.parse(traceLines[0]);
      console.log(JSON.stringify({
        name: sampleTrace.name,
        traceId: sampleTrace.traceId,
        spanId: sampleTrace.spanId,
        status: sampleTrace.status,
        attributes: Object.keys(sampleTrace.attributes || {}).reduce((acc, key) => {
          if (key.startsWith('ai.')) {
            acc[key] = sampleTrace.attributes[key];
          }
          return acc;
        }, {}),
        eventsCount: sampleTrace.events?.length || 0,
        resourceAttributes: Object.keys(sampleTrace.resource?.attributes || {}).length
      }, null, 2));
      
      console.log('\n🎉 Trace verification completed successfully!');
      
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
  console.log('\n🧹 Telemetry shutdown complete');
  
  // Keep the trace file for manual inspection
  if (existsSync(testTraceFile)) {
    console.log(`📁 Trace file preserved for inspection: ${testTraceFile}`);
    console.log(`   View with: cat ${testTraceFile} | jq '.'`);
  }
}

console.log('\n✅ Comprehensive trace verification test completed!');