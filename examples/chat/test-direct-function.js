// Test direct function call with telemetry
import { TelemetryConfig } from './telemetry.js';
import { trace } from '@opentelemetry/api';

// Initialize telemetry first
const telemetryConfig = new TelemetryConfig({
  enableFile: true,
  enableConsole: true,
  filePath: './direct-test-traces.jsonl'
});

telemetryConfig.initialize();

// Test function with tracing
function testFunction() {
  const tracer = trace.getTracer('direct-test');
  return tracer.startActiveSpan('testFunction', (span) => {
    try {
      console.log('🔍 Inside test function with span');
      
      span.setAttributes({
        'test.name': 'direct-function-test',
        'test.timestamp': Date.now()
      });
      
      const result = 'Test completed successfully';
      span.setStatus({ code: 1 }); // SUCCESS
      return result;
    } catch (error) {
      span.recordException(error);
      span.setStatus({ code: 2, message: error.message });
      throw error;
    } finally {
      span.end();
    }
  });
}

// Test the function
console.log('Testing direct function call with telemetry...');
const result = testFunction();
console.log('✅ Result:', result);

// Wait and shutdown
setTimeout(async () => {
  console.log('⏳ Shutting down telemetry...');
  await telemetryConfig.shutdown();
  console.log('🎉 Test completed!');
}, 2000);