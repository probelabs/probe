#!/usr/bin/env node

import { TelemetryConfig } from './telemetry.js';
import { appTracer } from './appTracer.js';
import { extractImageUrls } from './probeChat.js';
import { readFileSync, unlinkSync, existsSync } from 'fs';
import { createHash } from 'crypto';

/**
 * Test to verify proper trace hierarchy with consistent trace IDs
 */

console.log('🔍 Testing Trace Hierarchy\n');

const testTraceFile = './hierarchy-traces.jsonl';

// Clean up any existing test files
if (existsSync(testTraceFile)) {
  unlinkSync(testTraceFile);
}

// Initialize telemetry
const telemetryConfig = new TelemetryConfig({
  serviceName: 'probe-chat',
  serviceVersion: '1.0.0',
  enableFile: true,
  enableConsole: false,
  enableRemote: false,
  filePath: testTraceFile,
});

telemetryConfig.initialize();

console.log('✅ Telemetry initialized');

async function testTraceHierarchy() {
  const sessionId = 'test-session-123';
  const messageId = 'test-msg-456';
  const message = 'Here is an image: https://github.com/user-attachments/assets/example.png';
  
  // Calculate expected trace ID for verification
  const expectedTraceId = createHash('sha256').update(sessionId).digest('hex').substring(0, 32);
  console.log(`📋 Session ID: ${sessionId}`);
  console.log(`📋 Expected Trace ID: ${expectedTraceId}`);

  console.log('📝 Creating hierarchical test traces...');

  // Start a chat session
  const sessionSpan = appTracer.startChatSession(sessionId, message, 'anthropic', 'claude-3-7-sonnet-20250219');
  
  // Start user message processing within session context
  await appTracer.withSessionContext(sessionId, async () => {
    const userProcessingSpan = appTracer.startUserMessageProcessing(sessionId, messageId, message, 1);
    
    // Extract image URLs within user processing context
    const result = await appTracer.withUserProcessingContext(sessionId, async () => {
      return await extractImageUrls(message, true);
    });
    
    console.log('✅ extractImageUrls result:', result);
    
    // Start agent loop within user processing context
    appTracer.withUserProcessingContext(sessionId, () => {
      appTracer.startAgentLoop(sessionId, 5);
      
      // Start iteration within agent loop context
      appTracer.withAgentLoopContext(sessionId, () => {
        appTracer.startAgentIteration(sessionId, 1, 3, 1000);
        
        // Start AI request within iteration context
        appTracer.withIterationContext(sessionId, 1, () => {
          const aiRequestSpan = appTracer.startAiGenerationRequest(sessionId, 1, 'claude-3-7-sonnet-20250219', 'anthropic', {
            temperature: 0.3,
            maxTokens: 1000,
            maxRetries: 2
          });
          
          // End AI request
          appTracer.endAiRequest(sessionId, 1, true);
        });
        
        // End iteration
        appTracer.endIteration(sessionId, 1, true, 'completion');
      });
      
      // End agent loop
      appTracer.endAgentLoop(sessionId, 1, true, 'completion');
    });
    
    // End user processing
    appTracer.endUserMessageProcessing(sessionId, true);
  });
  
  // End chat session
  appTracer.endChatSession(sessionId, true, 1500);
  
  console.log('✅ All spans created and ended');
}

// Run the test
try {
  await testTraceHierarchy();
  
  // Force flush the spans
  await telemetryConfig.sdk.getTracerProvider().forceFlush();
  
  // Give it time to write to file
  await new Promise(resolve => setTimeout(resolve, 1000));
  
  console.log('\n🔍 Verifying trace hierarchy...');
  
  if (existsSync(testTraceFile)) {
    const traceData = readFileSync(testTraceFile, 'utf8');
    const traceLines = traceData.split('\n').filter(Boolean);
    
    console.log(`✅ Found ${traceLines.length} trace entries`);
    
    if (traceLines.length > 0) {
      const traces = traceLines.map(line => JSON.parse(line));
      
      // Group spans by trace ID
      const traceGroups = {};
      traces.forEach(trace => {
        if (!traceGroups[trace.traceId]) {
          traceGroups[trace.traceId] = [];
        }
        traceGroups[trace.traceId].push(trace);
      });
      
      console.log(`\n📊 Trace Analysis:`);
      console.log(`  ✓ Total spans: ${traces.length}`);
      console.log(`  ✓ Unique trace IDs: ${Object.keys(traceGroups).length}`);
      
      // Verify all spans belong to the same trace
      if (Object.keys(traceGroups).length === 1) {
        console.log(`  ✅ SUCCESS: All spans belong to the same trace ID!`);
        
        const traceId = Object.keys(traceGroups)[0];
        const spans = traceGroups[traceId];
        
        console.log(`\n🔗 Trace ID: ${traceId}`);
        console.log(`📋 Spans in hierarchy:`);
        
        // Sort spans by start time to show hierarchy
        spans.sort((a, b) => a.startTimeUnixNano - b.startTimeUnixNano);
        
        spans.forEach((span, index) => {
          const hasParent = span.parentSpanId ? '└─' : '┌─';
          console.log(`  ${hasParent} [${index + 1}] ${span.name}`);
          console.log(`      Span ID: ${span.spanId}`);
          if (span.parentSpanId) {
            console.log(`      Parent: ${span.parentSpanId}`);
          }
          console.log(`      Status: ${span.status?.code === 1 ? 'OK' : span.status?.code === 2 ? 'ERROR' : 'UNKNOWN'}`);
        });
        
        // Verify parent-child relationships
        let parentChildRelationships = 0;
        spans.forEach(span => {
          if (span.parentSpanId) {
            const parentExists = spans.some(s => s.spanId === span.parentSpanId);
            if (parentExists) {
              parentChildRelationships++;
            } else {
              console.log(`    ⚠️  Span ${span.name} has invalid parent reference`);
            }
          }
        });
        
        console.log(`\n✅ Parent-child relationships verified: ${parentChildRelationships}/${spans.length - 1} spans have valid parents`);
        
      } else {
        console.log(`  ❌ FAILURE: Spans are spread across ${Object.keys(traceGroups).length} different trace IDs`);
        
        Object.entries(traceGroups).forEach(([traceId, spans]) => {
          console.log(`    Trace ${traceId}: ${spans.length} spans`);
          spans.forEach(span => {
            console.log(`      - ${span.name}`);
          });
        });
      }
    }
    
    console.log('\n🎉 Hierarchy test completed!');
    
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
    console.log(`📁 Trace file preserved: ${testTraceFile}`);
  }
}

console.log('\n✅ Trace hierarchy test completed!');