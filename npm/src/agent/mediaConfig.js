/**
 * Shared media format configuration for Probe agent
 *
 * This module centralizes supported media formats (images + documents)
 * and their MIME types to ensure consistency across all components.
 *
 * Supports:
 * - Images: png, jpg, jpeg, webp, bmp, svg
 * - Documents: pdf (native support in Claude, Gemini, OpenAI via Vercel AI SDK)
 *
 * Note: GIF support was intentionally removed for compatibility with
 * AI models like Google Gemini that don't support animated images.
 */

// Supported image file extensions (without leading dot)
export const SUPPORTED_IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'svg'];

// Supported document file extensions (without leading dot)
export const SUPPORTED_DOCUMENT_EXTENSIONS = ['pdf'];

// All supported media extensions (images + documents)
export const SUPPORTED_MEDIA_EXTENSIONS = [...SUPPORTED_IMAGE_EXTENSIONS, ...SUPPORTED_DOCUMENT_EXTENSIONS];

// MIME type mapping for all supported media formats
export const MEDIA_MIME_TYPES = {
  'png': 'image/png',
  'jpg': 'image/jpeg',
  'jpeg': 'image/jpeg',
  'webp': 'image/webp',
  'bmp': 'image/bmp',
  'svg': 'image/svg+xml',
  'pdf': 'application/pdf'
};

// Legacy aliases for backward compatibility
export const IMAGE_MIME_TYPES = MEDIA_MIME_TYPES;

// Provider-specific unsupported media formats
export const PROVIDER_UNSUPPORTED_FORMATS = {
  'google': ['svg'],  // Google Gemini doesn't support image/svg+xml
};

/**
 * Check if a file extension is an image type
 * @param {string} extension - File extension (without dot)
 * @returns {boolean}
 */
export function isImageExtension(extension) {
  return SUPPORTED_IMAGE_EXTENSIONS.includes(extension?.toLowerCase());
}

/**
 * Check if a file extension is a document type (PDF, etc.)
 * @param {string} extension - File extension (without dot)
 * @returns {boolean}
 */
export function isDocumentExtension(extension) {
  return SUPPORTED_DOCUMENT_EXTENSIONS.includes(extension?.toLowerCase());
}

/**
 * Generate a regex pattern string for matching media file extensions
 * @param {string[]} extensions - Array of extensions (without dots)
 * @returns {string} Regex pattern string like "png|jpg|jpeg|webp|bmp|svg|pdf"
 */
export function getExtensionPattern(extensions = SUPPORTED_MEDIA_EXTENSIONS) {
  return extensions.join('|');
}

/**
 * Get MIME type for a file extension
 * @param {string} extension - File extension (without dot)
 * @returns {string|undefined} MIME type or undefined if not supported
 */
export function getMimeType(extension) {
  return MEDIA_MIME_TYPES[extension?.toLowerCase()];
}

/**
 * Check if a media extension is supported by a specific provider
 * @param {string} extension - File extension (without dot)
 * @param {string} provider - Provider name (e.g., 'google', 'anthropic', 'openai')
 * @returns {boolean} True if the format is supported by the provider
 */
export function isFormatSupportedByProvider(extension, provider) {
  if (!extension || typeof extension !== 'string') {
    return false;
  }
  if (extension.includes('/') || extension.includes('\\') || extension.includes('..')) {
    return false;
  }

  const ext = extension.toLowerCase();

  if (!SUPPORTED_MEDIA_EXTENSIONS.includes(ext)) {
    return false;
  }

  if (!provider || typeof provider !== 'string') {
    return true;
  }

  const unsupportedFormats = PROVIDER_UNSUPPORTED_FORMATS[provider];
  if (unsupportedFormats && unsupportedFormats.includes(ext)) {
    return false;
  }

  return true;
}

/**
 * Get supported media extensions for a specific provider
 * @param {string} provider - Provider name (e.g., 'google', 'anthropic', 'openai')
 * @returns {string[]} Array of supported extensions for this provider
 */
export function getSupportedExtensionsForProvider(provider) {
  if (!provider || typeof provider !== 'string') {
    return [...SUPPORTED_MEDIA_EXTENSIONS];
  }
  const unsupportedFormats = PROVIDER_UNSUPPORTED_FORMATS[provider] || [];
  return SUPPORTED_MEDIA_EXTENSIONS.filter(ext => !unsupportedFormats.includes(ext));
}
