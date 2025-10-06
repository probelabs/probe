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
        // Keep these as external dependencies that need to be installed
        '@modelcontextprotocol/sdk',
        '@ai-sdk/anthropic',
        '@ai-sdk/openai',
        '@ai-sdk/google',
        '@ai-sdk/amazon-bedrock',
        'ai',
        'glob',
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
        'process',
        // Additional problematic modules that use dynamic requires
        'fs-extra',
        'tar',
        'axios',
        'zod'
      ],
      banner: {
        js: '#!/usr/bin/env node'
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