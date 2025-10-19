# Test Suite for Bundled Binaries

This directory contains tests for the bundled binary extraction functionality.

## Test Files

### `extractor.test.js`
Unit tests for the binary extractor module (`src/extractor.js`).

**Coverage:**
- ✅ Platform detection (Linux, macOS, Windows)
- ✅ Unsupported platform error handling
- ✅ tar.gz archive extraction
- ✅ ZIP archive extraction (Windows)
- ✅ Path traversal security validation
- ✅ Error handling for missing binaries
- ✅ Error handling for empty archives

**Security Tests:**
- Path traversal attacks (../ sequences)
- Absolute path rejection
- Malicious archive handling

### `extractor-integration.test.js`
Integration tests that verify the extraction logic without requiring actual binary files.

**Coverage:**
- ✅ Platform detection logic for all 5 supported platforms
- ✅ Path safety validation
- ✅ Archive naming conventions
- ✅ Binary name detection (Windows vs Unix)
- ✅ Security validations

**Security Tests:**
- `isPathSafe()` logic verification
- Path normalization
- Relative path validation
- Directory traversal prevention

## Running Tests

```bash
# Run all tests
npm test

# Run with coverage
npm run test:coverage

# Run in watch mode
npm run test:watch

# Run verbose
npm run test:verbose
```

## Security Test Coverage

All security-critical functions have test coverage:

1. **Path Traversal Prevention** ✅
   - Tests verify `../ `sequences are rejected
   - Tests verify absolute paths are rejected
   - Tests verify safe relative paths are accepted

2. **Archive Extraction** ✅
   - tar.gz extraction with path validation
   - ZIP extraction with path validation
   - Malicious archive rejection

3. **Platform Detection** ✅
   - All 5 platforms correctly mapped
   - Unsupported platforms throw errors
   - Correct file extensions selected

## Test Dependencies

- `@jest/globals` - Test framework
- `fs-extra` - File system operations
- `tar` - tar.gz extraction
- `adm-zip` - ZIP extraction (dynamically imported)

## Notes

- Tests use dynamic imports for `adm-zip` to handle cases where it's not yet installed
- Tests skip platform-specific functionality (e.g., Windows ZIP tests on macOS)
- Security tests run on all platforms and verify the core logic
- Integration tests don't require actual binary files, only test the logic

## Coverage Goals

- ✅ Lines: >70%
- ✅ Functions: >70%
- ✅ Branches: >70%
- ✅ Statements: >70%

Security-critical functions should aim for 100% coverage.
