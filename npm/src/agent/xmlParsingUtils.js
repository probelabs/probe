/**
 * Shared XML parsing utilities used by both CLI/SDK and MCP modes
 * This module contains the core logic for thinking tag removal and attempt_complete recovery
 */

import { DEFAULT_VALID_TOOLS, buildToolTagPattern } from '../tools/common.js';

/**
 * Remove thinking tags and their content from XML string
 * Handles both closed and unclosed thinking tags
 * @param {string} xmlString - The XML string to clean
 * @returns {string} - Cleaned XML string without thinking tags
 */
export function removeThinkingTags(xmlString) {
  let result = xmlString;

  // Remove all properly closed thinking tags first
  result = result.replace(/<thinking>[\s\S]*?<\/thinking>/g, '');

  // Handle unclosed thinking tags
  // Find any remaining <thinking> tag (which means it's unclosed)
  const thinkingIndex = result.indexOf('<thinking>');
  if (thinkingIndex !== -1) {
    // Check if there's a tool tag after the thinking tag
    // We want to preserve tool tags even if they're after unclosed thinking
    const afterThinking = result.substring(thinkingIndex + '<thinking>'.length);

    // Look for any tool tags in the remaining content
    // Use the shared tool list to build the pattern dynamically
    const toolPattern = buildToolTagPattern(DEFAULT_VALID_TOOLS);
    const toolMatch = afterThinking.match(toolPattern);

    if (toolMatch) {
      // Found a tool tag - remove thinking tag and its content up to the tool tag
      const toolStart = thinkingIndex + '<thinking>'.length + toolMatch.index;
      result = result.substring(0, thinkingIndex) + result.substring(toolStart);
    } else {
      // No tool tag found - remove everything from <thinking> onwards
      result = result.substring(0, thinkingIndex);
    }
  }

  return result.trim();
}

/**
 * Extract thinking content for potential logging
 * Handles nested thinking tags by recursively stripping inner tags.
 * @param {string} xmlString - The XML string to extract from
 * @returns {string|null} - Thinking content (cleaned of nested tags) or null if not found
 */
export function extractThinkingContent(xmlString) {
  const thinkingMatch = xmlString.match(/<thinking>([\s\S]*?)<\/thinking>/);
  if (!thinkingMatch) {
    return null;
  }

  let content = thinkingMatch[1].trim();

  // Handle nested thinking tags: if the extracted content itself starts with <thinking>,
  // recursively extract from it until we get clean content.
  // This handles: <thinking><thinking>content</thinking></thinking>
  // where non-greedy match captures "<thinking>content" (issue #439)
  while (content.startsWith('<thinking>')) {
    const innerMatch = content.match(/<thinking>([\s\S]*?)<\/thinking>/);
    if (innerMatch) {
      content = innerMatch[1].trim();
    } else {
      // Unclosed inner <thinking> tag - strip the opening tag and use remaining content
      // e.g., "<thinking>content" becomes "content"
      content = content.substring('<thinking>'.length).trim();
      break;
    }
  }

  // Also strip any remaining thinking tags that might be embedded in the content
  content = content.replace(/<\/?thinking>/g, '').trim();

  return content || null;
}

/**
 * Check for attempt_complete recovery patterns and return standardized result
 * @param {string} cleanedXmlString - XML string with thinking tags already removed
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

/**
 * Apply the full thinking tag removal and attempt_complete recovery logic
 * This replicates the core logic from parseXmlToolCallWithThinking
 * @param {string} xmlString - The XML string to process
 * @param {Array<string>} validTools - List of valid tool names
 * @returns {Object} - Processing result with cleanedXml and potentialRecovery
 */
export function processXmlWithThinkingAndRecovery(xmlString, validTools = []) {
  // Extract thinking content if present (for potential logging or analysis)
  const thinkingContent = extractThinkingContent(xmlString);

  // Remove thinking tags and their content from the XML string
  const cleanedXmlString = removeThinkingTags(xmlString);

  // Check for attempt_complete recovery patterns
  const recoveryResult = checkAttemptCompleteRecovery(cleanedXmlString, validTools);

  // If debugging is enabled, log the thinking content
  if (process.env.DEBUG === '1' && thinkingContent) {
    console.log(`[DEBUG] AI Thinking Process:\n${thinkingContent}`);
  }

  return {
    cleanedXmlString,
    thinkingContent,
    recoveryResult
  };
}