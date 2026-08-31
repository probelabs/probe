import { closeSync, fsyncSync, openSync, renameSync, writeSync } from 'fs';

const mode = process.argv[2];
const modes = new Set(['normal', 'hung', 'crash', 'callback-failure', 'deadline']);
if (!modes.has(mode)) throw new Error('invalid mode');

if (mode === 'crash') process.exit(17);

const decision = Buffer.from(`${JSON.stringify({ id: 1, value: mode })}\n`);
if (mode === 'deadline') {
  const decisionFile = process.argv[3];
  if (!decisionFile) throw new Error('missing decision file');
  process.on('SIGTERM', () => {});
  const temporary = `${decisionFile}.tmp`;
  const descriptor = openSync(temporary, 'wx', 0o600);
  try {
    let offset = 0;
    while (offset < decision.length) {
      const written = writeSync(descriptor, decision, offset, decision.length - offset, offset);
      if (written <= 0) throw new Error('decision write made no progress');
      offset += written;
    }
    fsyncSync(descriptor);
  } finally {
    closeSync(descriptor);
  }
  renameSync(temporary, decisionFile);
}

process.stdout.write(decision);
if (mode === 'deadline') setInterval(() => {}, 1000);
