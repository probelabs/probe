#!/usr/bin/env node

// This file serves as the main entry point for the @buger/probe-web package
// It re-exports the main functionality from the parent directory

import path from 'path';
import { fileURLToPath } from 'url';

// Get the directory name of the current module
const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Export the main functionality
export * from '../main.js';

// Export the path to the web interface files
export const webInterfacePath = path.resolve(__dirname, '..');

console.log('Probe Web Interface loaded from:', webInterfacePath);