/**
 * Shared image format configuration for Probe agent
 *
 * This module centralizes supported image formats and their MIME types
 * to ensure consistency across all components.
 *
 * Note: GIF support was intentionally removed for compatibility with
 * AI models like Google Gemini that don't support animated images.
 */

// Supported image file extensions (without leading dot)
export const SUPPORTED_IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'svg'];

// MIME type mapping for supported image formats
export const IMAGE_MIME_TYPES = {
  'png': 'image/png',
  'jpg': 'image/jpeg',
  'jpeg': 'image/jpeg',
  'webp': 'image/webp',
  'bmp': 'image/bmp',
  'svg': 'image/svg+xml'
};

// Provider-specific unsupported image formats
// These providers do not support certain MIME types and will crash if they receive them
export const PROVIDER_UNSUPPORTED_FORMATS = {
  'google': ['svg'],  // Google Gemini doesn't support image/svg+xml
};

/**
 * Generate a regex pattern string for matching image file extensions
 * @param {string[]} extensions - Array of extensions (without dots)
 * @returns {string} Regex pattern string like "png|jpg|jpeg|webp|bmp|svg"
 */
export function getExtensionPattern(extensions = SUPPORTED_IMAGE_EXTENSIONS) {
  return extensions.join('|');
}

/**
 * Get MIME type for a file extension
 * @param {string} extension - File extension (without dot)
 * @returns {string|undefined} MIME type or undefined if not supported
 */
export function getMimeType(extension) {
  return IMAGE_MIME_TYPES[extension.toLowerCase()];
}

/**
 * Check if an image extension is supported by a specific provider
 * @param {string} extension - File extension (without dot)
 * @param {string} provider - Provider name (e.g., 'google', 'anthropic', 'openai')
 * @returns {boolean} True if the format is supported by the provider
 */
export function isFormatSupportedByProvider(extension, provider) {
  // Validate extension parameter - must be a non-empty string without path separators
  if (!extension || typeof extension !== 'string') {
    return false;
  }
  // Sanitize: reject extensions containing path traversal characters
  if (extension.includes('/') || extension.includes('\\') || extension.includes('..')) {
    return false;
  }

  const ext = extension.toLowerCase();

  // First check if it's a generally supported format
  if (!SUPPORTED_IMAGE_EXTENSIONS.includes(ext)) {
    return false;
  }

  // Handle null/undefined provider gracefully (treat as no restrictions)
  if (!provider || typeof provider !== 'string') {
    return true;
  }

  // Check provider-specific restrictions
  const unsupportedFormats = PROVIDER_UNSUPPORTED_FORMATS[provider];
  if (unsupportedFormats && unsupportedFormats.includes(ext)) {
    return false;
  }

  return true;
}

/**
 * Get supported image extensions for a specific provider
 * @param {string} provider - Provider name (e.g., 'google', 'anthropic', 'openai')
 * @returns {string[]} Array of supported extensions for this provider
 */
export function getSupportedExtensionsForProvider(provider) {
  // Handle null/undefined/non-string provider gracefully (return all extensions)
  if (!provider || typeof provider !== 'string') {
    return [...SUPPORTED_IMAGE_EXTENSIONS];
  }
  const unsupportedFormats = PROVIDER_UNSUPPORTED_FORMATS[provider] || [];
  return SUPPORTED_IMAGE_EXTENSIONS.filter(ext => !unsupportedFormats.includes(ext));
}
