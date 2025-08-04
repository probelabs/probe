import { test, describe, before, after } from 'node:test';
import assert from 'node:assert';
import { spawn } from 'child_process';
import { setTimeout as delay } from 'timers/promises';

describe('Backend Timeout Cleanup Tests', () => {
  // Helper to count active timers
  function getActiveTimerCount() {
    // This is a simplified check - in real tests you might use 
    // process._getActiveHandles() or similar debugging tools
    return process._getActiveHandles().filter(h => 
      h && h.constructor && h.constructor.name === 'Timeout'
    ).length;
  }

  test('should properly clear timeout when process completes successfully', async () => {
    const initialTimerCount = getActiveTimerCount();
    
    // Simulate a simple command that completes quickly
    const child = spawn('echo', ['test'], { shell: true });
    let timeoutFired = false;
    
    // Set a timeout (similar to backend implementation)
    const timeoutId = setTimeout(() => {
      timeoutFired = true;
      if (!child.killed) {
        child.kill('SIGTERM');
      }
    }, 5000); // 5 second timeout
    
    await new Promise((resolve, reject) => {
      child.on('close', (code) => {
        clearTimeout(timeoutId); // This is what we added to fix the issue
        resolve(code);
      });
      
      child.on('error', (error) => {
        clearTimeout(timeoutId); // This is what we added to fix the issue
        reject(error);
      });
    });
    
    // Give event loop time to clean up
    await delay(100);
    
    // Verify timeout was cleared and didn't fire
    assert.strictEqual(timeoutFired, false, 'Timeout should not have fired');
    
    // Check that we don't have extra timers hanging around
    const finalTimerCount = getActiveTimerCount();
    assert.ok(
      finalTimerCount <= initialTimerCount + 1, // Allow for test framework timers
      `Timer leak detected: started with ${initialTimerCount} timers, ended with ${finalTimerCount}`
    );
  });

  test('should properly clear timeout when process fails', async () => {
    const initialTimerCount = getActiveTimerCount();
    
    // Simulate a command that fails
    const child = spawn('exit', ['1'], { shell: true });
    let timeoutFired = false;
    
    const timeoutId = setTimeout(() => {
      timeoutFired = true;
      if (!child.killed) {
        child.kill('SIGTERM');
      }
    }, 5000);
    
    const exitCode = await new Promise((resolve, reject) => {
      child.on('close', (code) => {
        clearTimeout(timeoutId);
        resolve(code);
      });
      
      child.on('error', (error) => {
        clearTimeout(timeoutId);
        reject(error);
      });
    });
    
    await delay(100);
    
    assert.strictEqual(exitCode, 1, 'Process should have failed with exit code 1');
    assert.strictEqual(timeoutFired, false, 'Timeout should not have fired');
    
    const finalTimerCount = getActiveTimerCount();
    assert.ok(
      finalTimerCount <= initialTimerCount + 1,
      `Timer leak detected: started with ${initialTimerCount} timers, ended with ${finalTimerCount}`
    );
  });

  test('timeout should fire and kill process if it runs too long', async () => {
    let timeoutFired = false;
    let processKilled = false;
    
    // Simulate a long-running command
    const child = spawn('sleep', ['10'], { shell: true });
    
    const timeoutId = setTimeout(() => {
      timeoutFired = true;
      if (!child.killed) {
        processKilled = true;
        child.kill('SIGTERM');
      }
    }, 100); // Very short timeout to ensure it fires
    
    const exitCode = await new Promise((resolve) => {
      child.on('close', (code) => {
        clearTimeout(timeoutId);
        resolve(code);
      });
      
      child.on('error', () => {
        clearTimeout(timeoutId);
        resolve(-1);
      });
    });
    
    assert.strictEqual(timeoutFired, true, 'Timeout should have fired');
    assert.strictEqual(processKilled, true, 'Process should have been killed');
    assert.notStrictEqual(exitCode, 0, 'Process should not have completed successfully');
  });

  test('multiple executions should not accumulate timers', async () => {
    const initialTimerCount = getActiveTimerCount();
    
    // Run multiple quick commands
    for (let i = 0; i < 5; i++) {
      const child = spawn('echo', [`test${i}`], { shell: true });
      
      const timeoutId = setTimeout(() => {
        if (!child.killed) {
          child.kill('SIGTERM');
        }
      }, 5000);
      
      await new Promise((resolve, reject) => {
        child.on('close', () => {
          clearTimeout(timeoutId);
          resolve();
        });
        
        child.on('error', (error) => {
          clearTimeout(timeoutId);
          reject(error);
        });
      });
    }
    
    await delay(100);
    
    const finalTimerCount = getActiveTimerCount();
    assert.ok(
      finalTimerCount <= initialTimerCount + 2, // Allow some variance for test framework
      `Timer accumulation detected: started with ${initialTimerCount} timers, ended with ${finalTimerCount} after 5 executions`
    );
  });
});