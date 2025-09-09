#!/usr/bin/env node

const fs = require('fs-extra');
const path = require('path');
const { execSync } = require('child_process');

async function buildMcp() {
  try {
    console.log('Building MCP TypeScript...');
    
    // Ensure build directory exists
    await fs.ensureDir('build');
    
    // Copy src files to build directory
    console.log('Copying source files...');
    await fs.copy('src', 'build', { 
      overwrite: true,
      errorOnExist: false 
    });
    
    // Run TypeScript compiler
    console.log('Compiling TypeScript...');
    execSync('tsc src/mcp/index.ts --outDir build/mcp --module esnext --target es2020 --moduleResolution node --esModuleInterop --allowSyntheticDefaultImports --skipLibCheck', {
      stdio: 'inherit',
      cwd: process.cwd()
    });
    
    console.log('✅ MCP build completed successfully');
    
  } catch (error) {
    console.error('❌ MCP build failed:', error.message);
    process.exit(1);
  }
}

buildMcp();