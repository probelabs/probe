const esbuild = require('esbuild');
const path = require('path');
const fs = require('fs');

async function buildAgent() {
  try {
    console.log('Building agent...');
    
    // Ensure build directory exists
    const buildDir = path.resolve(__dirname, '..', 'build', 'agent');
    if (!fs.existsSync(buildDir)) {
      fs.mkdirSync(buildDir, { recursive: true });
    }

    const result = await esbuild.build({
      entryPoints: [path.resolve(__dirname, '..', 'src', 'agent', 'index.js')],
      bundle: true,
      outfile: path.resolve(buildDir, 'index.js'),
      platform: 'node',
      target: 'node18',
      format: 'esm',
      external: [
        // AI SDK packages - use dynamic requires, must be external
        '@modelcontextprotocol/sdk',
        '@ai-sdk/anthropic',
        '@ai-sdk/openai',
        '@ai-sdk/google',
        '@ai-sdk/amazon-bedrock',
        'ai',
        // Packages with dynamic requires
        'fs-extra',
        'tar',
        'axios',
        'dotenv',
        // Node.js built-in modules
        'fs',
        'path',
        'crypto',
        'util',
        'child_process',
        'stream',
        'events',
        'url',
        'os',
        'process'
        // Will bundle: glob, zod
      ],
      banner: {
        js: [
          '#!/usr/bin/env node',
          'import { createRequire } from "node:module";',
          'const require = createRequire(import.meta.url);'
        ].join('\n')
      },
      minify: false, // Keep readable for debugging
      sourcemap: false,
      metafile: true,
      logLevel: 'info'
    });

    // Make the output file executable
    fs.chmodSync(path.resolve(buildDir, 'index.js'), 0o755);
    
    console.log('Agent build completed successfully!');
    
    if (result.metafile) {
      // Optional: log build statistics
      const analysis = await esbuild.analyzeMetafile(result.metafile);
      console.log('Build analysis:');
      console.log(analysis);
    }
    
  } catch (error) {
    console.error('Agent build failed:', error);
    process.exit(1);
  }
}

buildAgent();
