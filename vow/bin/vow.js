#!/usr/bin/env node
'use strict';
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

function getRepoRoot() {
  try {
    const out = execSync('git rev-parse --show-toplevel', { stdio: ['ignore', 'pipe', 'ignore'] })
      .toString()
      .trim();
    if (out) return out;
  } catch (_) {}
  return process.cwd();
}

function main() {
  const repo = getRepoRoot();
  const consentFile = path.join(repo, '.AGENT_CONSENT');
  const localVow = path.join(repo, 'AGENT_VOW.md');
  const localConsent = path.join(repo, 'AGENT_CONSENT.md');
  const bundledVow = path.resolve(__dirname, '..', 'AGENT_VOW.md');

  // Determine which markdown to use: prefer AGENT_VOW.md, then AGENT_CONSENT.md, else bundled default
  let mdPath = null;
  if (fs.existsSync(localVow)) mdPath = localVow;
  else if (fs.existsSync(localConsent)) mdPath = localConsent;
  else if (fs.existsSync(bundledVow)) mdPath = bundledVow;

  let exitCode = 0;
  try {
    if (!mdPath) return; // No consent policy available anywhere
    const consentExists = fs.existsSync(consentFile);
    if (!consentExists) {
      try {
        const content = fs.readFileSync(mdPath, 'utf8');
        process.stderr.write(content + '\n');
      } catch (e) {
        process.stderr.write('Consent policy missing or unreadable.\n');
      }
      exitCode = 1;
    }
  } finally {
    // Cleanup consent file so each attempt requires fresh consent
    try {
      if (mdPath && fs.existsSync(consentFile)) {
        fs.unlinkSync(consentFile);
      }
    } catch (_) {}
    process.exit(exitCode);
  }
}

main();
