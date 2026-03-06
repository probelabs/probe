import { jest } from '@jest/globals';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const probeAgentPath = resolve(__dirname, '../../src/agent/ProbeAgent.js');
const delegatePath = resolve(__dirname, '../../src/delegate.js');

jest.unstable_mockModule(probeAgentPath, () => ({
  ProbeAgent: jest.fn()
}));

const { DelegationManager } = await import(delegatePath);

describe('Issue #487 - DelegationManager operational wait logging', () => {
  let manager;

  afterEach(() => {
    if (manager) {
      manager.cleanup();
      manager = null;
    }
  });

  test('logs when acquire() has to queue even when debug=false', async () => {
    manager = new DelegationManager({ maxConcurrent: 1, queueTimeout: 2000 });

    await manager.acquire('session-a', false);
    const queuedAcquire = manager.acquire('session-b', false, 2000);
    try {
      await new Promise(resolve => setTimeout(resolve, 5));

      expect(console.error).toHaveBeenCalledWith(
        expect.stringContaining('Slot unavailable')
      );
    } finally {
      manager.release('session-a', false);
      await queuedAcquire;
      manager.release('session-b', false);
    }
  });

  test('logs when queued item is granted even when debug=false', async () => {
    manager = new DelegationManager({ maxConcurrent: 1, queueTimeout: 2000 });

    await manager.acquire('session-a', false);
    const queuedAcquire = manager.acquire('session-b', false, 2000);
    try {
      await new Promise(resolve => setTimeout(resolve, 5));
      console.error.mockClear();

      manager.release('session-a', false);
      await queuedAcquire;

      expect(console.error).toHaveBeenCalledWith(
        expect.stringContaining('Granted slot from queue')
      );
    } finally {
      if (manager.getStats().globalActive > 0) {
        manager.release('session-b', false);
      }
    }
  });

  test('logs queued rejection on session-limit check even when debug=false', async () => {
    manager = new DelegationManager({
      maxConcurrent: 2,
      maxPerSession: 1,
      queueTimeout: 2000
    });

    await manager.acquire('session-a', false);
    await manager.acquire('session-b', false);

    const queuedAcquire = manager.acquire('session-a', false, 2000);
    await new Promise(resolve => setTimeout(resolve, 5));

    try {
      console.error.mockClear();
      manager.release('session-b', false);

      await expect(queuedAcquire).rejects.toThrow(/Maximum delegations per session/);
      expect(console.error).toHaveBeenCalledWith(
        expect.stringContaining('Session limit')
      );
    } finally {
      if (manager.getStats().globalActive > 0) {
        manager.release('session-a', false);
      }
    }
  });
});
