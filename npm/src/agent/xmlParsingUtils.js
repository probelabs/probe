/**
 * Shared XML parsing utilities used by both CLI/SDK and MCP modes
 * This module contains the core logic for attempt_complete recovery
 */

import { DEFAULT_VALID_TOOLS } from '../tools/common.js';

/**
 * Check for attempt_complete recovery patterns and return standardized result
 * @param {string} cleanedXmlString - XML string to check for attempt_complete patterns
 * @param {Array<string>} validTools - List of valid tool names
 * @returns {Object|null} - Standardized attempt_completion result or null
 */
export function checkAttemptCompleteRecovery(cleanedXmlString, validTools = []) {
  // Check for <attempt_completion> with content (with or without closing tag)
  // This handles: "<attempt_completion>content" or "<attempt_completion>content</attempt_completion>"

  // IMPORTANT: Use greedy match ([\s\S]*) instead of non-greedy ([\s\S]*?) to handle cases
  // where the content contains the string "</attempt_completion>" (e.g., in regex patterns or code examples).
  // We want to find the LAST occurrence of </attempt_completion>, not the first one.
  const openTagIndex = cleanedXmlString.indexOf('<attempt_completion>');
  if (openTagIndex !== -1) {
    const afterOpenTag = cleanedXmlString.substring(openTagIndex + '<attempt_completion>'.length);
    const closeTagIndex = cleanedXmlString.lastIndexOf('</attempt_completion>');

    let content;
    let hasClosingTag = false;

    if (closeTagIndex !== -1 && closeTagIndex >= openTagIndex + '<attempt_completion>'.length) {
      // Found a closing tag at or after the opening tag - extract content between them
      content = cleanedXmlString.substring(
        openTagIndex + '<attempt_completion>'.length,
        closeTagIndex
      ).trim();
      hasClosingTag = true;
    } else {
      // No closing tag - use content from opening tag to end of string
      content = afterOpenTag.trim();
      hasClosingTag = false;
    }

    if (content) {
      // If there's content after the tag, use it as the result
      return {
        toolName: 'attempt_completion',
        params: { result: content }
      };
    }

    // If the tag exists but is empty:
    // - With closing tag (e.g., "<attempt_completion></attempt_completion>"): use empty string
    // - Without closing tag (e.g., "<attempt_completion>"): use previous response
    return {
      toolName: 'attempt_completion',
      params: { result: hasClosingTag ? '' : '__PREVIOUS_RESPONSE__' }
    };
  }

  // Enhanced recovery logic for attempt_complete shorthand
  const attemptCompletePatterns = [
    // Standard shorthand with optional whitespace
    /^<attempt_complete>\s*$/,
    // Empty with proper closing tag (common case from the logs)
    /^<attempt_complete>\s*<\/attempt_complete>\s*$/,
    // Self-closing variant
    /^<attempt_complete\s*\/>\s*$/,
    // Incomplete opening tag (missing closing bracket)
    /^<attempt_complete\s*$/,
    // With trailing content (extract just the tag part) - must come after empty tag pattern
    /^<attempt_complete>(.*)$/s,
    // Self-closing with trailing content
    /^<attempt_complete\s*\/>(.*)$/s
  ];

  for (const pattern of attemptCompletePatterns) {
    const match = cleanedXmlString.match(pattern);
    if (match) {
      // Convert any form of attempt_complete to the standard format
      return {
        toolName: 'attempt_completion',
        params: { result: '__PREVIOUS_RESPONSE__' }
      };
    }
  }

  // Additional recovery: check if the string contains attempt_complete anywhere
  // and treat the entire response as a completion signal if no other tool tags are found
  if (cleanedXmlString.includes('<attempt_complete') && !hasOtherToolTags(cleanedXmlString, validTools)) {
    // This handles malformed cases where attempt_complete appears but is broken
    return {
      toolName: 'attempt_completion',
      params: { result: '__PREVIOUS_RESPONSE__' }
    };
  }

  return null;
}

/**
 * Helper function to check if the XML string contains other tool tags
 * @param {string} xmlString - The XML string to check
 * @param {string[]} validTools - List of valid tool names
 * @returns {boolean} - True if other tool tags are found
 */
function hasOtherToolTags(xmlString, validTools = []) {
  // Use the shared canonical tool list as default
  const toolsToCheck = validTools.length > 0 ? validTools : DEFAULT_VALID_TOOLS;
  
  // Check for any tool tags other than attempt_complete variants
  for (const tool of toolsToCheck) {
    if (tool !== 'attempt_completion' && xmlString.includes(`<${tool}`)) {
      return true;
    }
  }
  return false;
}

