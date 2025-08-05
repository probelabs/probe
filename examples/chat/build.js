#!/usr/bin/env node

import { build } from 'esbuild';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { existsSync, rmSync, mkdirSync, cpSync } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const isProduction = process.env.NODE_ENV === 'production';

async function buildApp() {
  console.log(`🏗️  Building probe-chat (${isProduction ? 'production' : 'development'})...`);
  
  // Clean dist directory
  const distDir = join(__dirname, 'dist');
  if (existsSync(distDir)) {
    rmSync(distDir, { recursive: true });
  }
  mkdirSync(distDir, { recursive: true });

  try {
    // Bundle the main CLI application
    await build({
      entryPoints: [join(__dirname, 'index.js')],
      bundle: true,
      platform: 'node',
      target: 'node18',
      format: 'esm',
      outfile: join(distDir, 'index.js'),
      external: [
        // Keep these as external since they have complex ESM/CJS interop issues
        '@opentelemetry/*',
        '@ai-sdk/*', 
        'tiktoken',
        // Keep probe as external since it downloads binaries at runtime
        '@buger/probe',
        // These have dynamic require issues when bundled
        'dotenv',
        'inquirer',
        'ora',
        'chalk',
        'commander',
        'zod',
        'ai',
        'glob'
      ],
      minify: isProduction,
      sourcemap: !isProduction,
      keepNames: true,
      define: {
        'process.env.NODE_ENV': JSON.stringify(process.env.NODE_ENV || 'development')
      },
      // Handle ESM imports properly
      mainFields: ['module', 'main'],
      conditions: ['import', 'module', 'default'],
      logLevel: 'info',
      // Preserve shebang from source file
      preserveSymlinks: true
    });

    // Copy essential static files
    const staticFiles = [
      'package.json',
      'index.html',
      'logo.png'
    ];

    for (const file of staticFiles) {
      const src = join(__dirname, file);
      const dest = join(distDir, file);
      if (existsSync(src)) {
        cpSync(src, dest);
        console.log(`📄 Copied ${file}`);
      }
    }

    // Copy directories that might be needed at runtime
    const staticDirs = [
      'bin',
      'storage', 
      'implement'
    ];

    for (const dir of staticDirs) {
      const src = join(__dirname, dir);
      const dest = join(distDir, dir);
      if (existsSync(src)) {
        cpSync(src, dest, { recursive: true });
        console.log(`📁 Copied ${dir}/`);
      }
    }

    console.log('✅ Build completed successfully!');
    console.log(`📦 Output: ${distDir}`);
    
  } catch (error) {
    console.error('❌ Build failed:', error);
    process.exit(1);
  }
}

buildApp();