#!/usr/bin/env node

/**
 * Simple test script for the pluggable backend system
 */

import { createImplementTool } from '../implement/core/ImplementTool.js';
import { listBackendNames, getBackendMetadata } from '../implement/backends/registry.js';

async function testBackends() {
  console.log('🧪 Testing Probe Chat Pluggable Backend System\n');
  
  // List available backends
  console.log('📋 Available Backends:');
  const backends = listBackendNames();
  for (const backend of backends) {
    const metadata = getBackendMetadata(backend);
    console.log(`\n  ${backend}:`);
    console.log(`    Version: ${metadata.version}`);
    console.log(`    Description: ${metadata.description}`);
    console.log(`    Languages: ${metadata.capabilities.supportsLanguages.join(', ')}`);
  }
  
  console.log('\n' + '='.repeat(50) + '\n');
  
  // Test backend initialization
  console.log('🔧 Testing Backend Initialization:\n');
  
  const tool = createImplementTool({
    enabled: true,
    backendConfig: {
      defaultBackend: 'aider',
      fallbackBackends: ['claude-code']
    }
  });
  
  try {
    const info = await tool.getInfo();
    console.log('✅ Backend system initialized successfully');
    console.log(`   Default backend: ${info.defaultBackend}`);
    console.log(`   Fallback backends: ${info.fallbackBackends.join(', ')}`);
    console.log(`   Available backends: ${info.availableBackends.join(', ')}`);
    
    console.log('\n📊 Backend Health Status:');
    for (const [name, health] of Object.entries(info.health)) {
      console.log(`   ${name}: ${health.status} ${health.available ? '✅' : '❌'}`);
    }
  } catch (error) {
    console.error('❌ Failed to initialize backend system:', error.message);
  }
  
  console.log('\n' + '='.repeat(50) + '\n');
  
  // Test a simple implementation request (dry run)
  console.log('🚀 Testing Implementation Request (Dry Run):\n');
  
  const testRequest = {
    task: 'Create a simple hello world function in JavaScript',
    dryRun: true,
    sessionId: 'test-' + Date.now()
  };
  
  console.log('Request:', testRequest);
  
  try {
    console.log('\nExecuting request...\n');
    const result = await tool.execute(testRequest);
    
    if (result.success) {
      console.log('✅ Request executed successfully');
      console.log(`   Backend used: ${result.backend}`);
      console.log(`   Execution time: ${result.metrics?.executionTime}ms`);
      console.log('\nOutput preview:');
      console.log(result.output?.substring(0, 200) + '...');
    } else {
      console.log('❌ Request failed');
      console.log(`   Error: ${result.error}`);
    }
  } catch (error) {
    console.error('❌ Error executing request:', error.message);
  }
  
  // Cleanup
  await tool.cleanup();
  
  console.log('\n✅ Test completed');
}

// Run the test
testBackends().catch(console.error);