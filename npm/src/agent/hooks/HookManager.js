/**
 * Hook manager for ProbeAgent
 * Enables event-driven integration with external systems
 */
export class HookManager {
  constructor() {
    this.hooks = new Map(); // hookName -> Set<callback>
  }

  /**
   * Register a hook callback
   * @param {string} hookName - Name of the hook
   * @param {Function} callback - Callback function
   * @returns {Function} Unregister function
   */
  on(hookName, callback) {
    if (!this.hooks.has(hookName)) {
      this.hooks.set(hookName, new Set());
    }
    this.hooks.get(hookName).add(callback);

    // Return unregister function
    return () => this.off(hookName, callback);
  }

  /**
   * Register a one-time hook callback
   * @param {string} hookName - Name of the hook
   * @param {Function} callback - Callback function
   * @returns {Function} Unregister function
   */
  once(hookName, callback) {
    const wrappedCallback = async (data) => {
      this.off(hookName, wrappedCallback);
      await callback(data);
    };
    return this.on(hookName, wrappedCallback);
  }

  /**
   * Unregister a hook callback
   * @param {string} hookName - Name of the hook
   * @param {Function} callback - Callback function
   */
  off(hookName, callback) {
    const callbacks = this.hooks.get(hookName);
    if (callbacks) {
      callbacks.delete(callback);
    }
  }

  /**
   * Emit a hook event
   * @param {string} hookName - Name of the hook
   * @param {any} data - Data to pass to callbacks
   * @returns {Promise<void>}
   */
  async emit(hookName, data) {
    const callbacks = this.hooks.get(hookName);
    if (!callbacks || callbacks.size === 0) return;

    // Execute all callbacks in parallel
    const promises = Array.from(callbacks).map(callback => {
      try {
        return Promise.resolve(callback(data));
      } catch (error) {
        console.error(`[HookManager] Error in hook ${hookName}:`, error);
        return Promise.resolve(); // Don't let one error break all hooks
      }
    });

    await Promise.all(promises);
  }

  /**
   * Clear all hooks or hooks for a specific event
   * @param {string} [hookName] - Optional hook name to clear
   */
  clear(hookName) {
    if (hookName) {
      this.hooks.delete(hookName);
    } else {
      this.hooks.clear();
    }
  }

  /**
   * Get list of registered hook names
   * @returns {string[]} Array of hook names
   */
  getHookNames() {
    return Array.from(this.hooks.keys());
  }

  /**
   * Get number of callbacks for a hook
   * @param {string} hookName - Name of the hook
   * @returns {number} Number of callbacks
   */
  getCallbackCount(hookName) {
    const callbacks = this.hooks.get(hookName);
    return callbacks ? callbacks.size : 0;
  }
}

/**
 * Available hook types
 * @type {Object<string, string>}
 */
export const HOOK_TYPES = {
  // Lifecycle hooks
  AGENT_INITIALIZED: 'agent:initialized',
  AGENT_CLEANUP: 'agent:cleanup',

  // Message hooks
  MESSAGE_USER: 'message:user',
  MESSAGE_ASSISTANT: 'message:assistant',
  MESSAGE_SYSTEM: 'message:system',

  // Tool execution hooks
  TOOL_START: 'tool:start',
  TOOL_END: 'tool:end',
  TOOL_ERROR: 'tool:error',

  // AI streaming hooks
  AI_STREAM_START: 'ai:stream:start',
  AI_STREAM_DELTA: 'ai:stream:delta',
  AI_STREAM_END: 'ai:stream:end',

  // Storage hooks
  STORAGE_LOAD: 'storage:load',
  STORAGE_SAVE: 'storage:save',
  STORAGE_CLEAR: 'storage:clear',

  // Iteration hooks
  ITERATION_START: 'iteration:start',
  ITERATION_END: 'iteration:end',
};
