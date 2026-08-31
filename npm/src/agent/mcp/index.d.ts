import type { ProbeAgent } from '../ProbeAgent.js';

export type MCPRecord = Record<string, unknown>;
export interface BuiltInMCPServerOptions { port?: number; host?: string; debug?: boolean;
  governedProfileVersion?: 'probe.governed-codex-profile/v2'; }
export interface GovernedCallEvidence { admitted: number; closed: number; overflow: boolean; }
export interface MCPToolDefinition { name: string; description: string; inputSchema: MCPRecord; }
export interface MCPToolResult { content: Array<{ type: string; text: string }>; isError?: boolean; }

export class BuiltInMCPServer {
  constructor(agent: ProbeAgent, options?: BuiltInMCPServerOptions);
  start(): Promise<{ host: string; port: number }>;
  stop(): Promise<void>;
  handleListTools(): Promise<{ tools: MCPToolDefinition[] }>;
  handleCallTool(params: { name: string; arguments?: MCPRecord }): Promise<MCPToolResult>;
  getGovernedCallEvidence(): Readonly<GovernedCallEvidence>;
  getToolCount(): number;
  getConfig(): { transport: 'http'; url: string };
}

export class MCPClientManager {
  constructor(options?: MCPRecord);
  initialize(config?: MCPRecord | null): Promise<MCPRecord>;
  connectToServer(config: MCPRecord): Promise<unknown>;
  callTool(toolName: string, args?: MCPRecord): Promise<unknown>;
  callGracefulStopAll(): Promise<unknown[]>;
  getTools(): Record<string, unknown>;
  getVercelTools(): Record<string, unknown>;
  disconnect(): Promise<void>;
}

export function createMCPManager(options?: MCPRecord): Promise<MCPClientManager>;
export function createTransport(serverConfig: MCPRecord): unknown;
export function loadMCPConfiguration(): MCPRecord;
export function loadMCPConfigurationFromPath(configPath: string): MCPRecord;
export function parseEnabledServers(config: MCPRecord): MCPRecord[];
export function createSampleConfig(): MCPRecord;
export function saveConfig(config: MCPRecord, path: string): void;

export class MCPXmlBridge {
  constructor(options?: MCPRecord);
  initialize(config?: MCPRecord | MCPRecord[] | null): Promise<void>;
  getVercelTools(filterToolNames?: string[] | null): Record<string, unknown>;
  getToolNames(): string[];
  isMcpTool(toolName: string): boolean;
  callGracefulStopAll(): Promise<unknown[]>;
  cleanup(): Promise<void>;
}

export function mcpToolToDescription(name: string, tool: MCPRecord): string;

declare const MCP: {
  MCPClientManager: typeof MCPClientManager;
  createMCPManager: typeof createMCPManager;
  createTransport: typeof createTransport;
  loadMCPConfiguration: typeof loadMCPConfiguration;
  loadMCPConfigurationFromPath: typeof loadMCPConfigurationFromPath;
  parseEnabledServers: typeof parseEnabledServers;
  createSampleConfig: typeof createSampleConfig;
  saveConfig: typeof saveConfig;
  MCPXmlBridge: typeof MCPXmlBridge;
  mcpToolToDescription: typeof mcpToolToDescription;
  BuiltInMCPServer: typeof BuiltInMCPServer;
};
export default MCP;
