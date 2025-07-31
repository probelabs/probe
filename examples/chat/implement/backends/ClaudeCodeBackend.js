/**
 * Claude Code SDK backend implementation
 * @module ClaudeCodeBackend
 */

import BaseBackend from './BaseBackend.js';
import { BackendError, ErrorTypes, ProgressTracker, FileChangeParser, TokenEstimator } from '../core/utils.js';
import { exec, spawn } from 'child_process';
import { promisify } from 'util';
import path from 'path';

const execPromise = promisify(exec);

/**
 * Claude Code SDK implementation backend
 * @class
 * @extends BaseBackend
 */
class ClaudeCodeBackend extends BaseBackend {
  constructor() {
    super('claude-code', '1.0.0');
    this.sdk = null;
    this.config = null;
  }

  /**
   * @override
   */
  async initialize(config) {
    this.config = {
      apiKey: config.apiKey || process.env.ANTHROPIC_API_KEY,
      model: config.model || 'claude-3-5-sonnet-20241022',
      baseUrl: config.baseUrl,
      timeout: config.timeout || 300000, // 5 minutes default
      maxTokens: config.maxTokens || 8000,
      temperature: config.temperature || 0.3,
      systemPrompt: config.systemPrompt,
      tools: config.tools || ['edit', 'search', 'bash'],
      maxTurns: config.maxTurns || 10,
      ...config
    };
    
    try {
      // Dynamically import Claude Code SDK
      // Try TypeScript version first, fall back to Python if needed
      try {
        const claudeCodeModule = await import('@anthropic-ai/claude-code');
        this.sdk = claudeCodeModule;
        this.sdkType = 'typescript';
        this.log('info', 'Initialized with TypeScript Claude Code SDK');
      } catch (tsError) {
        // TypeScript SDK not available, this is expected in JavaScript environment
        // We'll use the CLI interface instead
        this.sdkType = 'cli';
        this.log('info', 'Will use Claude Code CLI interface');
      }
      
      // Validate configuration
      await this.validateConfiguration();
      
      // Test connection/availability
      const available = await this.isAvailable();
      if (!available) {
        throw new Error('Claude Code is not available');
      }
      
      this.initialized = true;
      
    } catch (error) {
      throw new BackendError(
        `Failed to initialize Claude Code backend: ${error.message}`,
        ErrorTypes.INITIALIZATION_FAILED,
        'CLAUDE_CODE_INIT_FAILED',
        { originalError: error }
      );
    }
  }

  /**
   * @override
   */
  async isAvailable() {
    if (!this.config.apiKey) {
      this.log('warn', 'No API key configured');
      return false;
    }
    
    try {
      let claudeCommand = null;
      
      // Method 1: Try direct execution with claude --version
      try {
        await execPromise('claude --version', { timeout: 5000 });
        claudeCommand = 'claude';
        this.log('debug', 'Claude found in PATH via direct execution');
      } catch (directError) {
        this.log('debug', 'Claude not directly executable from PATH', { error: directError.message });
      }
      
      // Method 2: Check npm global installation and find the binary
      if (!claudeCommand) {
        try {
          const { stdout } = await execPromise('npm list -g @anthropic-ai/claude-code --depth=0', { timeout: 5000 });
          if (stdout.includes('@anthropic-ai/claude-code')) {
            // Get npm global bin directory
            const { stdout: binPath } = await execPromise('npm bin -g', { timeout: 5000 });
            const npmBinDir = binPath.trim();
            
            // Build the claude command path
            const isWindows = process.platform === 'win32';
            const claudeBinary = isWindows ? 'claude.cmd' : 'claude';
            const claudePath = path.join(npmBinDir, claudeBinary);
            
            // Test if we can execute it
            try {
              await execPromise(`"${claudePath}" --version`, { timeout: 5000 });
              claudeCommand = claudePath;
              
              // Update PATH for this process to include npm global bin
              const pathSeparator = isWindows ? ';' : ':';
              process.env.PATH = `${npmBinDir}${pathSeparator}${process.env.PATH}`;
              
              this.log('info', `Claude found at ${claudePath}, added ${npmBinDir} to PATH`);
            } catch (execError) {
              this.log('debug', `Failed to execute claude at ${claudePath}`, { error: execError.message });
            }
          }
        } catch (npmError) {
          this.log('debug', 'Failed to check npm global packages', { error: npmError.message });
        }
      }
      
      // Method 3: Try to find claude in common locations
      if (!claudeCommand) {
        const isWindows = process.platform === 'win32';
        const homeDir = process.env[isWindows ? 'USERPROFILE' : 'HOME'];
        const claudeBinary = isWindows ? 'claude.cmd' : 'claude';
        
        // Common npm global locations
        const commonPaths = [
          // Windows paths
          isWindows && path.join(process.env.APPDATA || '', 'npm', claudeBinary),
          isWindows && path.join('C:', 'Program Files', 'nodejs', claudeBinary),
          // Unix-like paths
          !isWindows && path.join('/usr/local/bin', claudeBinary),
          !isWindows && path.join(homeDir, '.npm-global', 'bin', claudeBinary),
          !isWindows && path.join(homeDir, '.local', 'bin', claudeBinary),
          // Cross-platform home directory paths
          path.join(homeDir, 'node_modules', '.bin', claudeBinary),
        ].filter(Boolean);
        
        for (const claudePath of commonPaths) {
          try {
            await execPromise(`"${claudePath}" --version`, { timeout: 2000 });
            claudeCommand = claudePath;
            this.log('info', `Claude found at ${claudePath}`);
            break;
          } catch (e) {
            // Continue searching
          }
        }
      }
      
      if (!claudeCommand) {
        this.log('warn', 'Claude Code CLI not found. Please install with: npm install -g @anthropic-ai/claude-code');
        return false;
      }
      
      // Store the command for later use
      this.claudeCommand = claudeCommand;
      
      // Just verify the API key exists (non-empty)
      // Don't validate format as it can vary
      if (!this.config.apiKey || this.config.apiKey.trim() === '') {
        this.log('warn', 'API key is not configured');
        return false;
      }
      
      return true;
    } catch (error) {
      this.log('debug', 'Availability check failed', { error: error.message });
      return false;
    }
  }

  /**
   * @override
   */
  getRequiredDependencies() {
    return [
      {
        name: '@anthropic-ai/claude-code',
        type: 'npm',
        version: '^1.0.0',
        installCommand: 'npm install @anthropic-ai/claude-code',
        description: 'Claude Code TypeScript SDK (optional)'
      },
      {
        name: 'claude-code',
        type: 'cli',
        installCommand: 'npm install -g @anthropic-ai/claude-code',
        description: 'Claude Code CLI tool'
      },
      {
        name: 'ANTHROPIC_API_KEY',
        type: 'environment',
        description: 'Anthropic API key for Claude Code SDK'
      }
    ];
  }

  /**
   * @override
   */
  getCapabilities() {
    return {
      supportsLanguages: ['javascript', 'typescript', 'python', 'rust', 'go', 'java', 'c++', 'c#', 'ruby', 'php', 'swift'],
      supportsStreaming: true,
      supportsRollback: false,
      supportsDirectFileEdit: true,
      supportsPlanGeneration: true,
      supportsTestGeneration: true,
      maxConcurrentSessions: 5
    };
  }

  /**
   * @override
   */
  getDescription() {
    return 'Claude Code SDK - Advanced AI coding assistant powered by Claude';
  }

  /**
   * @override
   */
  async execute(request) {
    this.checkInitialized();
    
    const validation = this.validateRequest(request);
    if (!validation.valid) {
      throw new BackendError(
        `Invalid request: ${validation.errors.join(', ')}`,
        ErrorTypes.VALIDATION_ERROR,
        'INVALID_REQUEST'
      );
    }
    
    const sessionInfo = this.createSessionInfo(request.sessionId);
    const progressTracker = new ProgressTracker(request.sessionId, request.callbacks?.onProgress);
    
    this.activeSessions.set(request.sessionId, sessionInfo);
    
    try {
      progressTracker.startStep('prepare', 'Preparing Claude Code execution');
      
      // Build the prompt
      const prompt = this.buildPrompt(request);
      const workingDir = request.context?.workingDirectory || process.cwd();
      
      this.updateSessionStatus(request.sessionId, {
        status: 'running',
        progress: 25,
        message: 'Claude Code is processing your request'
      });
      
      progressTracker.endStep();
      progressTracker.startStep('execute', 'Executing with Claude Code');
      
      let result;
      
      if (this.sdkType === 'typescript' && this.sdk) {
        // Use TypeScript SDK
        result = await this.executeWithSDK(prompt, request, sessionInfo, progressTracker);
      } else {
        // Use CLI interface
        result = await this.executeWithCLI(prompt, workingDir, request, sessionInfo, progressTracker);
      }
      
      progressTracker.endStep();
      
      this.updateSessionStatus(request.sessionId, {
        status: 'completed',
        progress: 100,
        message: 'Implementation completed successfully'
      });
      
      return result;
      
    } catch (error) {
      this.updateSessionStatus(request.sessionId, {
        status: 'failed',
        message: error.message
      });
      
      if (error instanceof BackendError) {
        throw error;
      }
      
      throw new BackendError(
        `Claude Code execution failed: ${error.message}`,
        ErrorTypes.EXECUTION_FAILED,
        'CLAUDE_CODE_EXECUTION_FAILED',
        { originalError: error, sessionId: request.sessionId }
      );
    } finally {
      this.activeSessions.delete(request.sessionId);
    }
  }

  /**
   * Validate configuration
   * @private
   */
  async validateConfiguration() {
    if (!this.config.apiKey) {
      throw new Error('API key is required. Set ANTHROPIC_API_KEY environment variable or provide apiKey in config');
    }
    
    // No format validation - API key formats can vary
    // Model validation removed - model names change frequently
    
    // Validate tools list
    if (this.config.tools && Array.isArray(this.config.tools)) {
      this.config.tools = this.validateTools(this.config.tools);
    }
  }

  /**
   * Build prompt for Claude Code
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {string} Formatted prompt
   * @private
   */
  buildPrompt(request) {
    let prompt = '';
    
    // Add context if provided
    if (request.context?.additionalContext) {
      prompt += `Context:\n${request.context.additionalContext}\n\n`;
    }
    
    // Add main task
    prompt += `Task:\n${request.task}\n`;
    
    // Add constraints
    if (request.context?.allowedFiles && request.context.allowedFiles.length > 0) {
      prompt += `\nOnly modify these files: ${request.context.allowedFiles.join(', ')}\n`;
    }
    
    if (request.context?.language) {
      prompt += `\nPrimary language: ${request.context.language}\n`;
    }
    
    // Add options
    if (request.options?.generateTests) {
      prompt += '\nAlso generate appropriate tests for the implemented functionality.\n';
    }
    
    if (request.options?.dryRun) {
      prompt += '\nThis is a dry run - describe what changes would be made without actually implementing them.\n';
    }
    
    return prompt.trim();
  }

  /**
   * Build system prompt for Claude Code
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {string} System prompt
   * @private
   */
  buildSystemPrompt(request) {
    if (this.config.systemPrompt) {
      return this.config.systemPrompt;
    }
    
    return `You are an expert software developer assistant using Claude Code. Your task is to implement code changes based on user requirements.

Key guidelines:
- Follow best practices for the detected programming language
- Write clean, maintainable, and well-documented code
- Include error handling where appropriate
- Consider edge cases and potential issues
- Generate tests when requested or when it would be beneficial
- Make minimal, focused changes that achieve the requested functionality
- Preserve existing code style and conventions

Working directory: ${request.context?.workingDirectory || process.cwd()}
${request.context?.allowedFiles ? `Allowed files: ${request.context.allowedFiles.join(', ')}` : ''}
${request.context?.language ? `Primary language: ${request.context.language}` : ''}`;
  }

  /**
   * Execute using TypeScript SDK
   * @private
   */
  async executeWithSDK(prompt, request, sessionInfo, progressTracker) {
    const startTime = Date.now();
    const { query, ClaudeCodeOptions } = this.sdk;
    
    const options = new ClaudeCodeOptions({
      max_turns: request.options?.maxTurns || this.config.maxTurns,
      system_prompt: this.buildSystemPrompt(request),
      model: this.config.model,
      temperature: request.options?.temperature || this.config.temperature,
      allowed_tools: this.config.tools
    });
    
    const messages = [];
    let cancelled = false;
    
    // Set up cancellation
    sessionInfo.cancel = () => {
      cancelled = true;
    };
    
    try {
      for await (const message of query({ prompt, options })) {
        if (cancelled) {
          throw new BackendError(
            'Implementation cancelled by user',
            ErrorTypes.CANCELLATION,
            'USER_CANCELLED'
          );
        }
        
        messages.push(message);
        
        // Report progress
        progressTracker.reportMessage(message, 'info');
        
        // Update progress periodically
        const elapsed = Date.now() - startTime;
        const progress = Math.min(25 + (messages.length * 5), 90);
        this.updateSessionStatus(request.sessionId, { progress });
      }
      
      const output = messages.join('\n');
      const executionTime = Date.now() - startTime;
      
      // Parse changes from output
      const changes = FileChangeParser.parseChanges(output, request.context?.workingDirectory);
      
      return {
        success: true,
        sessionId: request.sessionId,
        output,
        changes,
        metrics: {
          executionTime,
          tokensUsed: TokenEstimator.estimate(prompt + output),
          filesModified: changes.length,
          linesChanged: 0, // Would need more sophisticated parsing
          turns: messages.length
        },
        metadata: {
          model: this.config.model,
          sdkType: 'typescript'
        }
      };
      
    } catch (error) {
      if (error instanceof BackendError) {
        throw error;
      }
      
      throw new BackendError(
        `SDK execution failed: ${error.message}`,
        ErrorTypes.EXECUTION_FAILED,
        'SDK_EXECUTION_FAILED',
        { originalError: error }
      );
    }
  }

  /**
   * Execute using CLI interface
   * @private
   */
  async executeWithCLI(prompt, workingDir, request, sessionInfo, progressTracker) {
    const startTime = Date.now();
    
    // Build Claude Code CLI arguments securely
    const args = this.buildSecureCommandArgs(request);
    
    // Add the prompt using -p flag
    const validatedPrompt = this.validatePrompt(prompt);
    args.unshift('-p', validatedPrompt);
    
    this.log('info', 'Executing Claude Code CLI', {
      command: 'claude',
      args: args.slice(0, 5), // Log first few args only for security
      workingDir
    });
    
    return new Promise(async (resolve, reject) => {
      // Use spawn instead of exec for better security
      // Use the command we found during isAvailable() check
      let claudeCommand = this.claudeCommand || 'claude';
      
      // If we don't have a stored command, try to find it again
      if (!this.claudeCommand) {
        try {
          // Try direct execution first
          await execPromise('claude --version', { timeout: 1000 });
          claudeCommand = 'claude';
        } catch (e) {
          // Try to find it in npm global bin
          try {
            const { stdout: binPath } = await execPromise('npm bin -g', { timeout: 1000 });
            const isWindows = process.platform === 'win32';
            const claudeBinary = isWindows ? 'claude.cmd' : 'claude';
            const potentialClaudePath = path.join(binPath.trim(), claudeBinary);
            
            // Test if we can execute it
            await execPromise(`"${potentialClaudePath}" --version`, { timeout: 1000 });
            claudeCommand = potentialClaudePath;
            this.log('info', `Using claude from npm global: ${claudeCommand}`);
          } catch (npmError) {
            // Fall back to 'claude' and let it fail with a clear error
            this.log('warn', 'Could not find claude in npm global bin, attempting direct execution');
          }
        }
      }
      
      const child = spawn(claudeCommand, args, {
        cwd: workingDir,
        env: this.buildSecureEnvironment(),
        stdio: ['pipe', 'pipe', 'pipe'],
        shell: process.platform === 'win32' // Use shell on Windows for .cmd files
      });
      
      sessionInfo.childProcess = child;
      sessionInfo.cancel = () => {
        if (child && !child.killed) {
          child.kill('SIGTERM');
        }
      };
      
      let output = '';
      let errorOutput = '';
      
      // No need to send prompt to stdin - it's passed via -p argument
      if (child.stdin) {
        child.stdin.end();
      }
      
      // Handle stdout
      if (child.stdout) {
        child.stdout.on('data', (data) => {
          const chunk = data.toString();
          output += chunk;
          
          // Stream to stderr for visibility
          process.stderr.write(chunk);
          
          // Report progress
          progressTracker.reportMessage(chunk.trim(), 'stdout');
        });
      }
      
      // Handle stderr
      if (child.stderr) {
        child.stderr.on('data', (data) => {
          const chunk = data.toString();
          errorOutput += chunk;
          
          // Stream to stderr
          process.stderr.write(chunk);
          
          // Check for errors
          if (chunk.toLowerCase().includes('error')) {
            progressTracker.reportMessage(chunk.trim(), 'stderr');
          }
        });
      }
      
      // Handle completion
      child.on('close', (code) => {
        const executionTime = Date.now() - startTime;
        
        if (code === 0) {
          // Parse changes from output
          const changes = FileChangeParser.parseChanges(output, workingDir);
          
          resolve({
            success: true,
            sessionId: request.sessionId,
            output,
            changes,
            metrics: {
              executionTime,
              tokensUsed: TokenEstimator.estimate(prompt + output),
              filesModified: changes.length,
              linesChanged: 0,
              exitCode: code
            },
            metadata: {
              command: 'claude',
              args: args.slice(0, 5), // Limited args for security
              model: this.config.model,
              sdkType: 'cli'
            }
          });
        } else {
          reject(new BackendError(
            `Claude Code CLI exited with code ${code}`,
            ErrorTypes.EXECUTION_FAILED,
            'CLI_EXECUTION_FAILED',
            {
              exitCode: code,
              stdout: output.substring(0, 1000),
              stderr: errorOutput.substring(0, 1000)
            }
          ));
        }
      });
      
      // Handle errors
      child.on('error', (error) => {
        reject(new BackendError(
          `Failed to execute Claude Code CLI: ${error.message}`,
          ErrorTypes.EXECUTION_FAILED,
          'CLI_SPAWN_FAILED',
          { originalError: error }
        ));
      });
      
      // Set timeout
      const timeout = request.options?.timeout || this.config.timeout;
      setTimeout(() => {
        if (!child.killed) {
          child.kill('SIGTERM');
          reject(new BackendError(
            `Claude Code execution timed out after ${timeout}ms`,
            ErrorTypes.TIMEOUT,
            'CLAUDE_CODE_TIMEOUT',
            { timeout }
          ));
        }
      }, timeout);
    });
  }


  /**
   * Build secure command arguments
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {Array<string>} Secure command arguments
   * @private
   */
  buildSecureCommandArgs(request) {
    const args = [];

    // Add max turns with validation
    const maxTurns = this.validateMaxTurns(request.options?.maxTurns || this.config.maxTurns);
    args.push('--max-turns', maxTurns.toString());

    // Only add model if explicitly set
    const model = request.options?.model || this.config.model;
    if (model && this.isValidModelName(model)) {
      args.push('--model', model);
    }

    // Only add temperature if explicitly set
    const temperature = request.options?.temperature !== undefined ? request.options.temperature : this.config.temperature;
    if (temperature !== undefined && this.isValidTemperature(temperature)) {
      args.push('--temperature', temperature.toString());
    }

    // Add tools with validation or skip permissions if no tools
    const tools = request.options?.tools || this.config.tools;
    if (tools && tools.length > 0) {
      const validatedTools = this.validateTools(tools);
      if (validatedTools.length > 0) {
        args.push('--allowedTools', validatedTools.join(','));
      }
    } else {
      // If no tools are specified, add dangerously-skip-permissions flag
      args.push('--dangerously-skip-permissions');
    }

    return args;
  }

  /**
   * Build secure environment variables
   * @returns {Object} Secure environment variables
   * @private
   */
  buildSecureEnvironment() {
    const env = { ...process.env };
    
    if (this.config.apiKey && this.isValidApiKey(this.config.apiKey)) {
      env.ANTHROPIC_API_KEY = this.config.apiKey;
    }

    return env;
  }

  /**
   * Validate API key format
   * @param {string} apiKey - API key to validate
   * @returns {boolean} True if valid format
   * @private
   */
  isValidApiKey(apiKey) {
    // Just check if it's a non-empty string
    // API key formats can vary between providers and versions
    return apiKey && typeof apiKey === 'string' && apiKey.trim().length > 0;
  }

  /**
   * Validate model name
   * @param {string} model - Model name to validate
   * @returns {boolean} True if valid
   * @private
   */
  isValidModelName(model) {
    // Just check if it's a non-empty string
    // Model names change frequently and formats vary
    return model && typeof model === 'string' && model.trim().length > 0;
  }

  /**
   * Validate temperature value
   * @param {number} temperature - Temperature to validate
   * @returns {boolean} True if valid
   * @private
   */
  isValidTemperature(temperature) {
    return typeof temperature === 'number' && 
           temperature >= 0 && 
           temperature <= 2 &&
           !isNaN(temperature);
  }

  /**
   * Validate max turns value
   * @param {number} maxTurns - Max turns to validate
   * @returns {number} Validated max turns value
   * @private
   */
  validateMaxTurns(maxTurns) {
    if (typeof maxTurns !== 'number' || isNaN(maxTurns) || maxTurns < 1) {
      return 10; // Default value
    }
    
    return Math.min(Math.max(Math.floor(maxTurns), 1), 50); // Clamp between 1 and 50
  }

  /**
   * Validate tools list
   * @param {Array<string>} tools - Tools to validate
   * @returns {Array<string>} Validated tools
   * @private
   */
  validateTools(tools) {
    if (!Array.isArray(tools)) {
      return [];
    }

    const validTools = ['edit', 'search', 'bash', 'str_replace', 'create_file'];
    const validatedTools = [];

    for (const tool of tools) {
      if (typeof tool === 'string' && validTools.includes(tool)) {
        validatedTools.push(tool);
      } else {
        this.log('warn', `Invalid tool ignored: ${tool}`);
      }
    }

    return validatedTools;
  }

  /**
   * Validate prompt content
   * @param {string} prompt - Prompt to validate
   * @returns {string} Validated prompt
   * @private
   */
  validatePrompt(prompt) {
    if (!prompt || typeof prompt !== 'string') {
      throw new BackendError(
        'Invalid prompt content',
        ErrorTypes.VALIDATION_ERROR,
        'INVALID_PROMPT'
      );
    }

    const maxPromptLength = 100000; // 100KB limit for prompts
    
    if (prompt.length > maxPromptLength) {
      throw new BackendError(
        `Prompt too long (${prompt.length} chars, max: ${maxPromptLength})`,
        ErrorTypes.VALIDATION_ERROR,
        'PROMPT_TOO_LONG'
      );
    }

    // Check for control characters that could cause issues
    if (this.containsControlCharacters(prompt)) {
      this.log('warn', 'Prompt contains control characters, they will be filtered');
      return prompt.replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, ''); // Remove most control chars but keep newlines/tabs
    }

    return prompt;
  }

  /**
   * Check if string contains problematic control characters
   * @param {string} str - String to check
   * @returns {boolean} True if contains control characters
   * @private
   */
  containsControlCharacters(str) {
    // Check for control characters excluding newlines and tabs
    return /[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/.test(str);
  }
}

export default ClaudeCodeBackend;