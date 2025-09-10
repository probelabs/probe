/**
 * Utility functions for cleaning schema responses from AI models
 */

/**
 * Clean AI response by removing code blocks and format indicators
 * @param {string} response - Raw AI response
 * @returns {string} - Cleaned response
 */
export function cleanSchemaResponse(response) {
  if (!response || typeof response !== 'string') {
    return response;
  }

  let cleaned = response.trim();

  // Remove all markdown code blocks (including multiple blocks)
  // This handles ```json, ```xml, ```yaml, etc. and plain ```
  cleaned = cleaned.replace(/```\w*\s*\n?/g, '');
  cleaned = cleaned.replace(/\n?```\s*/g, '');

  // Remove any remaining leading/trailing whitespace
  cleaned = cleaned.trim();

  // Handle case where AI wrapped response in backticks without newlines
  // e.g., `{"result": "value"}` 
  if (cleaned.startsWith('`') && cleaned.endsWith('`')) {
    cleaned = cleaned.slice(1, -1).trim();
  }

  return cleaned;
}

/**
 * Validate that the cleaned response is valid JSON if expected
 * @param {string} response - Cleaned response
 * @returns {Object} - {isValid: boolean, parsed?: Object, error?: string}
 */
export function validateJsonResponse(response) {
  try {
    const parsed = JSON.parse(response);
    return { isValid: true, parsed };
  } catch (error) {
    return { isValid: false, error: error.message };
  }
}

/**
 * Validate that the cleaned response is valid XML if expected
 * @param {string} response - Cleaned response
 * @returns {Object} - {isValid: boolean, error?: string}
 */
export function validateXmlResponse(response) {
  // Basic XML validation - check for matching opening/closing tags
  const xmlPattern = /<\/?[\w\s="'.-]+>/g;
  const tags = response.match(xmlPattern);
  
  if (!tags) {
    return { isValid: false, error: 'No XML tags found' };
  }

  // Simple check for basic XML structure
  if (response.includes('<') && response.includes('>')) {
    return { isValid: true };
  }

  return { isValid: false, error: 'Invalid XML structure' };
}

/**
 * Process schema response with cleaning and optional validation
 * @param {string} response - Raw AI response
 * @param {string} schema - Original schema for context
 * @param {Object} options - Processing options
 * @returns {Object} - {cleaned: string, validation?: Object}
 */
export function processSchemaResponse(response, schema, options = {}) {
  const { validateJson = false, validateXml = false, debug = false } = options;

  // Clean the response
  const cleaned = cleanSchemaResponse(response);

  const result = { cleaned };

  if (debug) {
    result.debug = {
      originalLength: response.length,
      cleanedLength: cleaned.length,
      wasModified: response !== cleaned,
      removedContent: response !== cleaned ? {
        before: response.substring(0, 50) + (response.length > 50 ? '...' : ''),
        after: cleaned.substring(0, 50) + (cleaned.length > 50 ? '...' : '')
      } : null
    };
  }

  // Optional validation
  if (validateJson) {
    result.jsonValidation = validateJsonResponse(cleaned);
  }

  if (validateXml) {
    result.xmlValidation = validateXmlResponse(cleaned);
  }

  return result;
}