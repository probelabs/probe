#!/usr/bin/env node

import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

console.log('🔍 Verifying Probe Chat Integration Tests\n');

const tests = [
    {
        name: 'Chat Flow Tests',
        file: 'test/integration/chatFlows.test.js',
        expectedTests: 12
    },
    {
        name: 'Tool Calling Tests', 
        file: 'test/integration/toolCalling.test.js',
        expectedTests: 13
    }
];

async function runTest(testInfo) {
    return new Promise((resolve) => {
        console.log(`Running ${testInfo.name}...`);
        
        const child = spawn('node', [
            '--test',
            '--test-timeout=30000',
            testInfo.file
        ], {
            cwd: join(__dirname, '..'),
            stdio: 'pipe'
        });
        
        let output = '';
        let errorOutput = '';
        
        child.stdout.on('data', (data) => {
            output += data.toString();
        });
        
        child.stderr.on('data', (data) => {
            errorOutput += data.toString();
        });
        
        child.on('close', (code) => {
            const passMatch = output.match(/# pass (\d+)/);
            const failMatch = output.match(/# fail (\d+)/);
            const testsMatch = output.match(/# tests (\d+)/);
            const durationMatch = output.match(/# duration_ms ([\d.]+)/);
            
            const passed = passMatch ? parseInt(passMatch[1]) : 0;
            const failed = failMatch ? parseInt(failMatch[1]) : 0;
            const total = testsMatch ? parseInt(testsMatch[1]) : 0;
            const duration = durationMatch ? parseFloat(durationMatch[1]) : 0;
            
            resolve({
                ...testInfo,
                code,
                passed,
                failed,
                total,
                duration,
                errorOutput
            });
        });
    });
}

async function main() {
    const results = [];
    
    for (const test of tests) {
        const result = await runTest(test);
        results.push(result);
        
        if (result.code === 0 && result.failed === 0 && result.total === result.expectedTests) {
            console.log(`✅ ${test.name}: ${result.passed}/${result.total} tests passed (${(result.duration / 1000).toFixed(2)}s)`);
        } else {
            console.log(`❌ ${test.name}: ${result.passed}/${result.total} tests passed, ${result.failed} failed`);
            if (result.errorOutput) {
                console.log(`   Error: ${result.errorOutput.split('\n')[0]}`);
            }
        }
        console.log('');
    }
    
    console.log('📊 Summary:');
    const totalPassed = results.reduce((sum, r) => sum + r.passed, 0);
    const totalFailed = results.reduce((sum, r) => sum + r.failed, 0);
    const totalTests = results.reduce((sum, r) => sum + r.total, 0);
    const totalDuration = results.reduce((sum, r) => sum + r.duration, 0);
    
    console.log(`Total tests: ${totalTests}`);
    console.log(`Passed: ${totalPassed}`);
    console.log(`Failed: ${totalFailed}`);
    console.log(`Total duration: ${(totalDuration / 1000).toFixed(2)}s`);
    
    if (totalFailed === 0 && totalTests === 25) {
        console.log('\n✅ All integration tests are working correctly!');
        console.log('\nKey features verified:');
        console.log('- Mock LLM provider with streaming support');
        console.log('- Mock backend for testing');
        console.log('- Multi-turn conversations');
        console.log('- Tool calling and execution');
        console.log('- Error handling and recovery');
        console.log('- Timeout handling');
        process.exit(0);
    } else {
        console.log('\n❌ Some tests are failing or count mismatch');
        process.exit(1);
    }
}

main().catch(console.error);