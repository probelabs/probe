/**
 * Centralized timeout configuration for the implementation system
 * 
 * This file defines all timeout values used across the implementation pipeline
 * to ensure consistency and maintainability.
 */

export const TIMEOUTS = {
  // Main implementation timeouts (in seconds - user-friendly)
  IMPLEMENT_DEFAULT: 1200,        // 20 minutes - default for Claude Code/Aider execution
  IMPLEMENT_MINIMUM: 60,          // 1 minute - minimum allowed timeout
  IMPLEMENT_MAXIMUM: 3600,        // 1 hour - maximum allowed timeout

  // Quick verification checks (in milliseconds)
  VERSION_CHECK: 5000,            // 5 seconds - claude --version, aider --version
  PATH_CHECK: 2000,               // 2 seconds - command existence checks
  NPM_CHECK: 5000,                // 5 seconds - npm operations
  WSL_CHECK: 2000,                // 2 seconds - WSL availability checks
  
  // Network operations (in milliseconds) 
  HTTP_REQUEST: 10000,            // 10 seconds - GitHub URLs, remote requests
  FILE_FLUSH: 5000,               // 5 seconds - file operations and flushing
};

/**
 * Convert seconds to milliseconds for internal use
 * @param {number} seconds - Timeout in seconds
 * @returns {number} Timeout in milliseconds
 */
export function secondsToMs(seconds) {
  return seconds * 1000;
}

/**
 * Convert milliseconds to seconds for user display
 * @param {number} milliseconds - Timeout in milliseconds
 * @returns {number} Timeout in seconds
 */
export function msToSeconds(milliseconds) {
  return Math.floor(milliseconds / 1000);
}

/**
 * Validate timeout value is within acceptable bounds
 * @param {number} seconds - Timeout in seconds
 * @returns {boolean} True if valid
 */
export function isValidTimeout(seconds) {
  return seconds >= TIMEOUTS.IMPLEMENT_MINIMUM && seconds <= TIMEOUTS.IMPLEMENT_MAXIMUM;
}

/**
 * Get default timeout in milliseconds for internal use
 * @returns {number} Default timeout in milliseconds
 */
export function getDefaultTimeoutMs() {
  return secondsToMs(TIMEOUTS.IMPLEMENT_DEFAULT);
}