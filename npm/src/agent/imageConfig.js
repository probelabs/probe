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
