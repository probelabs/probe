/// <reference types="node" />

export type GovernedSignalScope = 'child' | 'process-group';
export type GovernedProcessClassification =
  | 'exited'
  | 'terminated'
  | 'aborted'
  | 'execution_timeout'
  | 'output_overflow'
  | 'cleanup_timeout'
  | 'spawn_error';

export interface GovernedProcessSpec {
  command: string;
  args?: string[];
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  signal?: AbortSignal;
  executionTimeoutMs?: number;
  terminationGraceMs?: number;
  cleanupTimeoutMs?: number;
  stdoutByteCap?: number;
  stderrByteCap?: number;
  signalScope?: GovernedSignalScope;
}

export interface GovernedProcessBarrierState {
  close: boolean;
  stdoutEOF: boolean;
  stderrEOF: boolean;
}

export interface GovernedProcessObservation {
  sequence: number;
  fact: string;
  [detail: string]: unknown;
}

export interface GovernedProcessReceipt {
  id: string;
  classification: GovernedProcessClassification;
  reason: string | null;
  error?: string;
  stdout: string;
  stderr: string;
  stdoutBytes: number;
  stderrBytes: number;
  exitCode: number | null;
  signal: NodeJS.Signals | null;
  barriers: Readonly<GovernedProcessBarrierState>;
  observed: readonly Readonly<GovernedProcessObservation>[];
}

export interface GovernedProcessHandle {
  readonly id: string;
  terminate(reason?: string): Promise<GovernedProcessReceipt>;
  readonly result: Promise<GovernedProcessReceipt>;
}

export function spawnGovernedProcess(spec: GovernedProcessSpec): GovernedProcessHandle;

export interface AcknowledgedJsonlRecord {
  id: number;
  value: string;
}

export interface AcknowledgedJsonlChannelOptions {
  onRecord(record: AcknowledgedJsonlRecord, signal: AbortSignal): unknown | PromiseLike<unknown>;
  frameByteCap?: number;
  totalByteCap?: number;
  idleTimeoutMs?: number;
  deadlineMs?: number;
  highWaterMark?: number;
}

export interface AcknowledgedJsonlChannelResult {
  classification: string;
  error: string | null;
  frames: number;
  acknowledgements: number;
  eof: boolean;
}

export interface AcknowledgedJsonlChannelSnapshot {
  accepting: boolean;
  eof: boolean;
  frames: number;
  acknowledgements: number;
  totalBytes: number;
  partialBytes: number;
  pending: number;
  writes: number;
  timers: number;
  drainWaiters: number;
  backpressureCount: number;
  firstFailureCount: number;
  laterFailureCount: number;
  abortCount: number;
  listeners: number;
  cleaned: boolean;
}

export interface AcknowledgedJsonlChannel {
  write(input: Buffer | Uint8Array | string): Promise<boolean>;
  end(): Promise<void>;
  readonly result: Promise<Readonly<AcknowledgedJsonlChannelResult>>;
  cleanup(): Promise<void>;
  snapshot(): AcknowledgedJsonlChannelSnapshot;
}

export function createAcknowledgedJsonlChannel(
  options: AcknowledgedJsonlChannelOptions
): Readonly<AcknowledgedJsonlChannel>;

export interface AtomicTerminalReceiptOptions {
  directory: string;
  name?: string;
  bytes: Buffer | Uint8Array | string;
  maxBytes?: number;
}

export interface AtomicTerminalReceiptResult {
  bytes: Buffer;
  mode: number;
  size: number;
}

export function writeAtomicTerminalReceipt(
  options: AtomicTerminalReceiptOptions
): Promise<Readonly<AtomicTerminalReceiptResult>>;

export const GOVERNED_CODEX_EXEC_PROTOCOL: 'probe.governed-codex-exec/v1';
export const GOVERNED_CODEX_EXEC_ATTESTATION_VERSION: 'probe.governed-codex-exec-attestation/v1';
export const GOVERNED_CODEX_EXEC_TRANSPORT: 'exec-jsonl-default-auth-v1';
export function buildGovernedCodexExecLaunch(options: Record<string, unknown>): Readonly<Record<string, unknown>>;
export function createGovernedCodexExecEngine(options: Record<string, unknown>): Promise<Readonly<{
  run(prompt?: string, options?: Record<string, unknown>): Promise<Readonly<Record<string, unknown>>>;
  query(prompt: string, options?: Record<string, unknown>): AsyncGenerator<Readonly<Record<string, unknown>>>;
  close(): Promise<void>;
  readonly launch: Readonly<Record<string, unknown>> | null;
}>>;
export function runGovernedCodexExec(options: Record<string, unknown>): Promise<Readonly<Record<string, unknown>>>;
export function formatGovernedCodexExecAttestation(input: Record<string, unknown>): Readonly<Record<string, unknown>>;
export const buildGovernedCodexExecAttestation: typeof formatGovernedCodexExecAttestation;
export function validateGovernedCodexExecAttestation(input: Record<string, unknown>): Readonly<Record<string, unknown>>;
export function projectGovernedCodexExecFailure(error: unknown): Readonly<Record<string, unknown>> | null;
export function normalizeGovernedCodexExecFailure(error: unknown, boundary: 'acquire' | 'query' | 'close'): Error;
export function previewGovernedCodexExecDispatch(prompt: string, systemPrompt?: string): Readonly<{ source: 'probe-host-exec'; tool: 'codex-exec'; promptDigest: `sha256:${string}`; promptBytes: number }>;
