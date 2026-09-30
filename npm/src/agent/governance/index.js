/**
 * Public ESM entry point for Probe's accepted process-governance primitives.
 * This module deliberately has no dependency on the package root or dotenv.
 */

export { spawnGovernedProcess } from '../processSupervisor.js';
export { createAcknowledgedJsonlChannel } from './acknowledgedJsonlChannel.js';
export { writeAtomicTerminalReceipt } from './atomicTerminalReceipt.js';
export {
  GOVERNED_CODEX_EXEC_PROTOCOL,
  GOVERNED_CODEX_EXEC_ATTESTATION_VERSION,
  GOVERNED_CODEX_EXEC_TRANSPORT,
  buildGovernedCodexExecLaunch,
  createGovernedCodexExecEngine,
  runGovernedCodexExec,
  formatGovernedCodexExecAttestation,
  buildGovernedCodexExecAttestation,
  validateGovernedCodexExecAttestation,
  projectGovernedCodexExecFailure,
  normalizeGovernedCodexExecFailure,
  previewGovernedCodexExecDispatch,
} from '../engines/governed-codex-exec.js';
