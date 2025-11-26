import { describe, test, expect } from '@jest/globals';
import {
  SUPPORTED_IMAGE_EXTENSIONS,
  IMAGE_MIME_TYPES,
  PROVIDER_UNSUPPORTED_FORMATS,
  getMimeType,
  isFormatSupportedByProvider,
  getSupportedExtensionsForProvider,
  getExtensionPattern
} from '../../src/agent/imageConfig.js';

describe('imageConfig', () => {
  describe('Constants', () => {
    test('SUPPORTED_IMAGE_EXTENSIONS should include standard formats', () => {
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('png');
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('jpg');
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('jpeg');
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('webp');
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('bmp');
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('svg');
    });

    test('IMAGE_MIME_TYPES should map extensions to correct MIME types', () => {
      expect(IMAGE_MIME_TYPES.png).toBe('image/png');
      expect(IMAGE_MIME_TYPES.jpg).toBe('image/jpeg');
      expect(IMAGE_MIME_TYPES.jpeg).toBe('image/jpeg');
      expect(IMAGE_MIME_TYPES.webp).toBe('image/webp');
      expect(IMAGE_MIME_TYPES.bmp).toBe('image/bmp');
      expect(IMAGE_MIME_TYPES.svg).toBe('image/svg+xml');
    });

    test('PROVIDER_UNSUPPORTED_FORMATS should have google provider restrictions', () => {
      expect(PROVIDER_UNSUPPORTED_FORMATS.google).toBeDefined();
      expect(PROVIDER_UNSUPPORTED_FORMATS.google).toContain('svg');
    });
  });

  describe('getMimeType', () => {
    test('should return correct MIME type for supported extensions', () => {
      expect(getMimeType('png')).toBe('image/png');
      expect(getMimeType('jpg')).toBe('image/jpeg');
      expect(getMimeType('svg')).toBe('image/svg+xml');
    });

    test('should handle case-insensitive extensions', () => {
      expect(getMimeType('PNG')).toBe('image/png');
      expect(getMimeType('JPG')).toBe('image/jpeg');
      expect(getMimeType('SVG')).toBe('image/svg+xml');
    });

    test('should return undefined for unsupported extensions', () => {
      expect(getMimeType('gif')).toBeUndefined();
      expect(getMimeType('tiff')).toBeUndefined();
      expect(getMimeType('pdf')).toBeUndefined();
    });
  });

  describe('isFormatSupportedByProvider', () => {
    test('should return true for formats supported by all providers', () => {
      expect(isFormatSupportedByProvider('png', 'google')).toBe(true);
      expect(isFormatSupportedByProvider('jpg', 'google')).toBe(true);
      expect(isFormatSupportedByProvider('jpeg', 'google')).toBe(true);
      expect(isFormatSupportedByProvider('webp', 'google')).toBe(true);
    });

    test('should return false for SVG with Google provider (GitHub issue #305)', () => {
      expect(isFormatSupportedByProvider('svg', 'google')).toBe(false);
      expect(isFormatSupportedByProvider('SVG', 'google')).toBe(false);
    });

    test('should return true for SVG with providers that support it', () => {
      expect(isFormatSupportedByProvider('svg', 'anthropic')).toBe(true);
      expect(isFormatSupportedByProvider('svg', 'openai')).toBe(true);
    });

    test('should return true for SVG with unknown providers (permissive default)', () => {
      expect(isFormatSupportedByProvider('svg', 'unknown-provider')).toBe(true);
    });

    test('should return false for completely unsupported formats', () => {
      expect(isFormatSupportedByProvider('gif', 'google')).toBe(false);
      expect(isFormatSupportedByProvider('tiff', 'anthropic')).toBe(false);
    });

    test('should handle case-insensitive extensions', () => {
      expect(isFormatSupportedByProvider('PNG', 'google')).toBe(true);
      expect(isFormatSupportedByProvider('SVG', 'google')).toBe(false);
    });

    test('should handle null/undefined provider gracefully', () => {
      expect(isFormatSupportedByProvider('png', null)).toBe(true);
      expect(isFormatSupportedByProvider('png', undefined)).toBe(true);
      expect(isFormatSupportedByProvider('svg', null)).toBe(true);
    });

    test('should reject invalid extension parameters', () => {
      expect(isFormatSupportedByProvider(null, 'google')).toBe(false);
      expect(isFormatSupportedByProvider(undefined, 'google')).toBe(false);
      expect(isFormatSupportedByProvider('', 'google')).toBe(false);
    });

    test('should reject path traversal attempts in extension', () => {
      expect(isFormatSupportedByProvider('../../../etc/passwd', 'google')).toBe(false);
      expect(isFormatSupportedByProvider('png/../../../etc/passwd', 'google')).toBe(false);
      expect(isFormatSupportedByProvider('..\\..\\windows\\system32', 'google')).toBe(false);
    });
  });

  describe('getSupportedExtensionsForProvider', () => {
    test('should return all extensions for providers without restrictions', () => {
      const anthropicExtensions = getSupportedExtensionsForProvider('anthropic');
      expect(anthropicExtensions).toEqual(SUPPORTED_IMAGE_EXTENSIONS);
    });

    test('should exclude SVG for Google provider', () => {
      const googleExtensions = getSupportedExtensionsForProvider('google');
      expect(googleExtensions).not.toContain('svg');
      expect(googleExtensions).toContain('png');
      expect(googleExtensions).toContain('jpg');
    });

    test('should return all extensions for unknown providers', () => {
      const unknownExtensions = getSupportedExtensionsForProvider('unknown');
      expect(unknownExtensions).toEqual(SUPPORTED_IMAGE_EXTENSIONS);
    });

    test('should handle null/undefined provider gracefully', () => {
      const nullExtensions = getSupportedExtensionsForProvider(null);
      const undefinedExtensions = getSupportedExtensionsForProvider(undefined);
      expect(nullExtensions).toEqual(SUPPORTED_IMAGE_EXTENSIONS);
      expect(undefinedExtensions).toEqual(SUPPORTED_IMAGE_EXTENSIONS);
    });
  });

  describe('getExtensionPattern', () => {
    test('should generate regex pattern for default extensions', () => {
      const pattern = getExtensionPattern();
      expect(pattern).toContain('png');
      expect(pattern).toContain('jpg');
      expect(pattern).toContain('svg');
      expect(pattern).toBe('png|jpg|jpeg|webp|bmp|svg');
    });

    test('should generate pattern for custom extensions', () => {
      const pattern = getExtensionPattern(['png', 'jpg']);
      expect(pattern).toBe('png|jpg');
    });
  });
});
