/**
 * Backend registry for automatic discovery and registration
 * @module registry
 */

const AiderBackend = require('./AiderBackend.js');
const ClaudeCodeBackend = require('./ClaudeCodeBackend.js');

/**
 * Available backend classes
 */
const AVAILABLE_BACKENDS = {
  aider: AiderBackend,
  'claude-code': ClaudeCodeBackend
};

/**
 * Get all available backend classes
 * @returns {Object<string, typeof BaseBackend>}
 */
function getAvailableBackends() {
  return { ...AVAILABLE_BACKENDS };
}

/**
 * Create a backend instance by name
 * @param {string} name - Backend name
 * @returns {BaseBackend|null}
 */
function createBackend(name) {
  const BackendClass = AVAILABLE_BACKENDS[name];
  if (!BackendClass) {
    return null;
  }
  
  return new BackendClass();
}

/**
 * Register a custom backend class
 * @param {string} name - Backend name
 * @param {typeof BaseBackend} BackendClass - Backend class
 */
function registerBackend(name, BackendClass) {
  AVAILABLE_BACKENDS[name] = BackendClass;
}

/**
 * Get backend metadata
 * @param {string} name - Backend name
 * @returns {Object|null}
 */
function getBackendMetadata(name) {
  const backend = createBackend(name);
  if (!backend) {
    return null;
  }
  
  return {
    name: backend.name,
    version: backend.version,
    description: backend.getDescription(),
    capabilities: backend.getCapabilities(),
    dependencies: backend.getRequiredDependencies()
  };
}

/**
 * List all registered backend names
 * @returns {string[]}
 */
function listBackendNames() {
  return Object.keys(AVAILABLE_BACKENDS);
}

module.exports = {
  getAvailableBackends,
  createBackend,
  registerBackend,
  getBackendMetadata,
  listBackendNames,
  // Export backend classes for direct use
  AiderBackend,
  ClaudeCodeBackend
};