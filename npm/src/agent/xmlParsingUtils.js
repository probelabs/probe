/**
 * Shared XML parsing utilities used by both CLI/SDK and MCP modes
 * This module contains the core logic for thinking tag removal and attempt_complete recovery
 */

/**
 * Remove thinking tags and their content from XML string
 * @param {string} xmlString - The XML string to clean
 * @returns {string} - Cleaned XML string without thinking tags
 */
export function removeThinkingTags(xmlString) {
  return xmlString.replace(/<thinking>[\s\S]*?<\/thinking>/g, '').trim();
}

/**
 * Extract thinking content for potential logging
 * @param {string} xmlString - The XML string to extract from
 * @returns {string|null} - Thinking content or null if not found
 */
export function extractThinkingContent(xmlString) {
  const thinkingMatch = xmlString.match(/<thinking>([\s\S]*?)<\/thinking>/);
  return thinkingMatch ? thinkingMatch[1].trim() : null;
}

/**
 * Check for attempt_complete recovery patterns and return standardized result
 * @param {string} cleanedXmlString - XML string with thinking tags already removed
 * @param {Array<string>} validTools - List of valid tool names
 * @returns {Object|null} - Standardized attempt_completion result or null
 */
export function checkAttemptCompleteRecovery(cleanedXmlString, validTools = []) {
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
  const defaultTools = ['search', 'query', 'extract', 'listFiles', 'searchFiles', 'implement', 'attempt_completion'];
  const toolsToCheck = validTools.length > 0 ? validTools : defaultTools;
  
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