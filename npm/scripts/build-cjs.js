#!/usr/bin/env node

import { promises as fs } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import esbuild from 'esbuild';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const npmDir = join(__dirname, '..');
const cjsDir = join(npmDir, 'cjs');

// Clean CJS directory
try {
  await fs.rm(cjsDir, { recursive: true, force: true });
} catch (err) {
  // Directory might not exist
}

// Create CJS directory structure
await fs.mkdir(cjsDir, { recursive: true });
await fs.mkdir(join(cjsDir, 'agent'), { recursive: true });

console.log('Building CommonJS modules...');

// Build main index file
await esbuild.build({
  entryPoints: [join(npmDir, 'src/index.js')],
  bundle: true,
  platform: 'node',
  format: 'cjs',
  outfile: join(cjsDir, 'index.cjs'),
  external: [
    '@ai-sdk/anthropic',
    '@ai-sdk/openai', 
    '@ai-sdk/google',
    '@modelcontextprotocol/sdk',
    '@opentelemetry/*',
    'ai',
    'axios',
    'fs-extra',
    'tar',
    'crypto',
    'fs',
    'path',
    'events',
    'child_process',
    'stream',
    'util',
    'os'
  ],
  define: {
    'import.meta.url': '"file:///"'
  }
});

// Build ProbeAgent separately
await esbuild.build({
  entryPoints: [join(npmDir, 'src/agent/ProbeAgent.js')],
  bundle: true,
  platform: 'node',
  format: 'cjs',
  outfile: join(cjsDir, 'agent/ProbeAgent.cjs'),
  external: [
    '@ai-sdk/anthropic',
    '@ai-sdk/openai',
    '@ai-sdk/google', 
    '@modelcontextprotocol/sdk',
    '@opentelemetry/*',
    'ai',
    'axios',
    'fs-extra',
    'tar',
    'crypto',
    'fs',
    'path',
    'events',
    'child_process',
    'stream',
    'util',
    'os'
  ],
  define: {
    'import.meta.url': '"file:///"'
  }
});

// Create package.json for CJS directory
const cjsPackageJson = {
  type: 'commonjs'
};

await fs.writeFile(
  join(cjsDir, 'package.json'),
  JSON.stringify(cjsPackageJson, null, 2)
);

console.log('✅ CommonJS build completed');
console.log(`   📁 Built to: ${cjsDir}`);
console.log('   📦 Main: cjs/index.cjs');
console.log('   🤖 Agent: cjs/agent/ProbeAgent.cjs');