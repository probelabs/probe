import { writeFileSync } from 'fs';

const mode = process.argv[2];
const readyFile = process.argv[3];
if ((mode !== 'normal' && mode !== 'hung') || !readyFile) throw new Error('invalid fixture arguments');

if (mode === 'hung') process.on('SIGTERM', () => {});
writeFileSync(readyFile, 'READY');

if (mode === 'normal') {
  process.stdout.write('normal-worker');
} else {
  setInterval(() => {}, 1000);
}
