/**
 * Tests for ESM import compatibility
 * Ensures all OpenTelemetry imports use named exports instead of default exports
 * to maintain compatibility with strict ESM environments (Bun, Next.js Turbopack, etc.)
 * @module tests/unit/esm-imports
 */

import { describe, test, expect } from '@jest/globals';
import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('ESM Import Compatibility', () => {
  describe('telemetry.js', () => {
    const telemetryPath = join(__dirname, '../../src/agent/telemetry.js');
    let telemetryContent;

    beforeAll(() => {
      telemetryContent = readFileSync(telemetryPath, 'utf-8');
    });

    test('should not use default imports for @opentelemetry/sdk-node', () => {
      // Check for default import pattern
      const defaultImportPattern = /import\s+\w+\s+from\s+['"]@opentelemetry\/sdk-node['"]/;
      expect(telemetryContent).not.toMatch(defaultImportPattern);

      // Verify named import is used
      expect(telemetryContent).toMatch(/import\s+\{\s*NodeSDK\s*\}\s+from\s+['"]@opentelemetry\/sdk-node['"]/);
    });

    test('should not use default imports for @opentelemetry/resources', () => {
      const defaultImportPattern = /import\s+\w+\s+from\s+['"]@opentelemetry\/resources['"]/;
      expect(telemetryContent).not.toMatch(defaultImportPattern);

      expect(telemetryContent).toMatch(/import\s+\{\s*resourceFromAttributes\s*\}\s+from\s+['"]@opentelemetry\/resources['"]/);
    });

    test('should not use default imports for @opentelemetry/exporter-trace-otlp-http', () => {
      const defaultImportPattern = /import\s+\w+\s+from\s+['"]@opentelemetry\/exporter-trace-otlp-http['"]/;
      expect(telemetryContent).not.toMatch(defaultImportPattern);

      expect(telemetryContent).toMatch(/import\s+\{\s*OTLPTraceExporter\s*\}\s+from\s+['"]@opentelemetry\/exporter-trace-otlp-http['"]/);
    });

    test('should not use default imports for @opentelemetry/sdk-trace-base', () => {
      const defaultImportPattern = /import\s+\w+\s+from\s+['"]@opentelemetry\/sdk-trace-base['"]/;
      expect(telemetryContent).not.toMatch(defaultImportPattern);

      expect(telemetryContent).toMatch(/import\s+\{\s*BatchSpanProcessor\s*,\s*ConsoleSpanExporter\s*\}\s+from\s+['"]@opentelemetry\/sdk-trace-base['"]/);
    });

    test('should not have intermediate destructuring of default imports', () => {
      // Check that we don't have patterns like:
      // const { NodeSDK } = nodeSDKPkg;
      const destructuringPatterns = [
        /const\s+\{\s*NodeSDK\s*\}\s*=\s*\w+Pkg/,
        /const\s+\{\s*resourceFromAttributes\s*\}\s*=\s*\w+Pkg/,
        /const\s+\{\s*OTLPTraceExporter\s*\}\s*=\s*\w+Pkg/,
        /const\s+\{\s*BatchSpanProcessor\s*,\s*ConsoleSpanExporter\s*\}\s*=\s*\w+Pkg/,
      ];

      destructuringPatterns.forEach((pattern) => {
        expect(telemetryContent).not.toMatch(pattern);
      });
    });

    test('should be valid ESM module', () => {
      // Verify it doesn't use require() or module.exports
      expect(telemetryContent).not.toMatch(/\brequire\s*\(/);
      expect(telemetryContent).not.toMatch(/module\.exports/);
      expect(telemetryContent).not.toMatch(/exports\./);
    });
  });

  describe('fileSpanExporter.js', () => {
    const exporterPath = join(__dirname, '../../src/agent/fileSpanExporter.js');
    let exporterContent;

    beforeAll(() => {
      exporterContent = readFileSync(exporterPath, 'utf-8');
    });

    test('should not use default imports for @opentelemetry/core', () => {
      const defaultImportPattern = /import\s+\w+\s+from\s+['"]@opentelemetry\/core['"]/;
      expect(exporterContent).not.toMatch(defaultImportPattern);

      expect(exporterContent).toMatch(/import\s+\{\s*ExportResultCode\s*\}\s+from\s+['"]@opentelemetry\/core['"]/);
    });

    test('should not have intermediate destructuring of default imports', () => {
      expect(exporterContent).not.toMatch(/const\s+\{\s*ExportResultCode\s*\}\s*=\s*\w+Pkg/);
    });
  });

  describe('General ESM patterns', () => {
    test('should verify key OpenTelemetry packages support named exports', async () => {
      // This test attempts to import the packages to ensure they work in strict ESM
      // If default imports were used, this would fail in strict ESM environments

      const { NodeSDK } = await import('@opentelemetry/sdk-node');
      const { resourceFromAttributes } = await import('@opentelemetry/resources');
      const { OTLPTraceExporter } = await import('@opentelemetry/exporter-trace-otlp-http');
      const { BatchSpanProcessor, ConsoleSpanExporter } = await import('@opentelemetry/sdk-trace-base');
      const { ExportResultCode } = await import('@opentelemetry/core');

      // Verify they are constructors/enums/functions
      expect(typeof NodeSDK).toBe('function');
      expect(typeof resourceFromAttributes).toBe('function');
      expect(typeof OTLPTraceExporter).toBe('function');
      expect(typeof BatchSpanProcessor).toBe('function');
      expect(typeof ConsoleSpanExporter).toBe('function');
      expect(ExportResultCode).toBeDefined();
    });
  });
});
