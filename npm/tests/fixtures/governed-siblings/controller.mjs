const mode = process.argv[2];
if (mode !== 'normal' && mode !== 'hung') throw new Error('invalid mode');
process.stdout.write(`${JSON.stringify({ id: 1, value: mode })}\n`);
