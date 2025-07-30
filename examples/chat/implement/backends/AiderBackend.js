/**
 * Aider backend implementation for code implementation tasks
 * @module AiderBackend
 */

import BaseBackend from './BaseBackend.js';
import { BackendError, ErrorTypes, ProgressTracker, FileChangeParser, TokenEstimator } from '../core/utils.js';
import { spawn, exec } from 'child_process';
import { promisify } from 'util';
import { promises as fsPromises } from 'fs';
import path from 'path';
import os from 'os';

const execPromise = promisify(exec);

/**
 * Aider implementation backend
 * @class
 * @extends BaseBackend
 */
class AiderBackend extends BaseBackend {
  constructor() {
    super('aider', '1.0.0');
    this.config = null;
    this.aiderVersion = null;
  }

  /**
   * @override
   */
  async initialize(config) {
    this.config = {
      command: 'aider',
      timeout: 300000, // 5 minutes default
      maxOutputSize: 10 * 1024 * 1024, // 10MB
      additionalArgs: [],
      environment: {},
      autoCommit: false,
      modelSelection: 'auto',
      ...config
    };
    
    // Test aider availability
    const available = await this.isAvailable();
    if (!available) {
      throw new BackendError(
        'Aider command not found or not accessible. Please install aider with: pip install aider-chat',
        ErrorTypes.DEPENDENCY_MISSING,
        'AIDER_NOT_FOUND'
      );
    }
    
    // Get aider version
    try {
      const { stdout } = await execPromise('aider --version', { timeout: 5000 });
      this.aiderVersion = stdout.trim();
      this.log('info', `Initialized with aider version: ${this.aiderVersion}`);
    } catch (error) {
      this.log('warn', 'Could not determine aider version', { error: error.message });
    }
    
    this.initialized = true;
  }

  /**
   * @override
   */
  async isAvailable() {
    try {
      // Test if aider command exists
      await execPromise('which aider', { timeout: 5000 });
      
      // Check if API key is available
      const hasApiKey = !!(
        process.env.ANTHROPIC_API_KEY ||
        process.env.OPENAI_API_KEY ||
        process.env.GOOGLE_API_KEY ||
        process.env.GEMINI_API_KEY
      );
      
      if (!hasApiKey) {
        this.log('warn', 'No API key found. Aider requires ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY');
        return false;
      }
      
      return true;
    } catch (error) {
      return false;
    }
  }

  /**
   * @override
   */
  getRequiredDependencies() {
    return [
      {
        name: 'aider-chat',
        type: 'pip',
        version: '>=0.20.0',
        installCommand: 'pip install aider-chat',
        description: 'AI pair programming tool'
      },
      {
        name: 'API Key',
        type: 'environment',
        description: 'One of: ANTHROPIC_API_KEY, OPENAI_API_KEY, GOOGLE_API_KEY, or GEMINI_API_KEY'
      }
    ];
  }

  /**
   * @override
   */
  getCapabilities() {
    return {
      supportsLanguages: ['python', 'javascript', 'typescript', 'go', 'rust', 'java', 'cpp', 'c', 'csharp', 'ruby', 'php', 'swift'],
      supportsStreaming: true,
      supportsRollback: true,
      supportsDirectFileEdit: true,
      supportsPlanGeneration: false,
      supportsTestGeneration: false,
      maxConcurrentSessions: 3
    };
  }

  /**
   * @override
   */
  getDescription() {
    return 'Aider - AI pair programming in your terminal';
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
      progressTracker.startStep('prepare', 'Preparing aider execution');
      
      // Create temporary file for task
      const tempDir = os.tmpdir();
      const tempFileName = `aider-task-${request.sessionId}-${Date.now()}.txt`;
      const tempFilePath = path.join(tempDir, tempFileName);
      
      await fsPromises.writeFile(tempFilePath, request.task, 'utf8');
      sessionInfo.tempFile = tempFilePath;
      
      this.log('debug', 'Created temporary task file', { path: tempFilePath });
      
      progressTracker.endStep();
      progressTracker.startStep('execute', 'Executing aider');
      
      // Build and execute aider command
      const command = this.buildCommand(request, tempFilePath);
      const workingDir = request.context?.workingDirectory || process.cwd();
      
      this.updateSessionStatus(request.sessionId, {
        status: 'running',
        progress: 25,
        message: 'Aider is processing your request'
      });
      
      // Execute aider
      const result = await this.executeCommand(command, workingDir, request, sessionInfo, progressTracker);
      
      progressTracker.endStep();
      
      // Clean up temp file
      try {
        await fsPromises.unlink(tempFilePath);
      } catch (error) {
        this.log('warn', 'Failed to clean up temp file', { path: tempFilePath, error: error.message });
      }
      
      this.updateSessionStatus(request.sessionId, {
        status: 'completed',
        progress: 100,
        message: 'Implementation completed successfully'
      });
      
      return result;
      
    } catch (error) {
      // Clean up temp file on error
      if (sessionInfo.tempFile) {
        try {
          await fsPromises.unlink(sessionInfo.tempFile);
        } catch (cleanupError) {
          this.log('warn', 'Failed to clean up temp file on error', { error: cleanupError.message });
        }
      }
      
      this.updateSessionStatus(request.sessionId, {
        status: 'failed',
        message: error.message
      });
      
      if (error instanceof BackendError) {
        throw error;
      }
      
      throw new BackendError(
        `Aider execution failed: ${error.message}`,
        ErrorTypes.EXECUTION_FAILED,
        'AIDER_EXECUTION_FAILED',
        { originalError: error, sessionId: request.sessionId }
      );
    } finally {
      this.activeSessions.delete(request.sessionId);
    }
  }

  /**
   * Build aider command with arguments
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @param {string} tempFilePath - Path to temporary file with task
   * @returns {string} Complete aider command
   * @private
   */
  buildCommand(request, tempFilePath) {
    const args = [
      '--yes',
      '--no-check-update',
      '--no-analytics',
      `--message-file "${tempFilePath}"`
    ];
    
    // Handle auto-commit option
    if (!request.options?.autoCommit && !this.config.autoCommit) {
      args.push('--no-auto-commits');
    }
    
    // Add model selection
    const model = this.selectModel(request);
    if (model) {
      args.push('--model', model);
    }
    
    // Add timeout if specified
    if (request.options?.timeout || this.config.timeout) {
      const timeoutSeconds = Math.floor((request.options?.timeout || this.config.timeout) / 1000);
      // Note: aider doesn't have a built-in timeout, this would need to be handled at process level
    }
    
    // Add additional arguments from config
    if (this.config.additionalArgs && this.config.additionalArgs.length > 0) {
      args.push(...this.config.additionalArgs);
    }
    
    // Add any custom arguments from request
    if (request.options?.additionalArgs) {
      args.push(...request.options.additionalArgs);
    }
    
    return `${this.config.command} ${args.join(' ')}`;
  }

  /**
   * Select the appropriate model based on configuration and environment
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {string|null} Model identifier or null
   * @private
   */
  selectModel(request) {
    // Priority: request option > config > environment-based auto-selection
    if (request.options?.model) {
      return request.options.model;
    }
    
    if (this.config.model) {
      return this.config.model;
    }
    
    if (this.config.modelSelection === 'auto') {
      // Auto-select based on available API keys
      const geminiApiKey = process.env.GEMINI_API_KEY || process.env.GOOGLE_API_KEY;
      const anthropicApiKey = process.env.ANTHROPIC_API_KEY;
      const openaiApiKey = process.env.OPENAI_API_KEY;
      
      if (geminiApiKey) {
        return 'gemini/gemini-2.5-pro';
      } else if (anthropicApiKey) {
        return 'claude-3-5-sonnet-20241022';
      } else if (openaiApiKey) {
        return 'gpt-4';
      }
    }
    
    return null;
  }

  /**
   * Execute aider command
   * @param {string} command - Command to execute
   * @param {string} workingDir - Working directory
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @param {Object} sessionInfo - Session information
   * @param {ProgressTracker} progressTracker - Progress tracker
   * @returns {Promise<import('../types/BackendTypes').ImplementResult>}
   * @private
   */
  async executeCommand(command, workingDir, request, sessionInfo, progressTracker) {
    return new Promise((resolve, reject) => {
      const startTime = Date.now();
      
      this.log('info', 'Executing aider command', {
        command: command.substring(0, 100) + '...',
        workingDir
      });
      
      // Spawn the process
      const childProcess = spawn('sh', ['-c', command], {
        cwd: workingDir,
        env: { ...process.env, ...this.config.environment }
      });
      
      sessionInfo.childProcess = childProcess;
      sessionInfo.cancel = () => {
        if (childProcess && !childProcess.killed) {
          this.log('info', 'Cancelling aider process', { sessionId: request.sessionId });
          childProcess.kill('SIGTERM');
          setTimeout(() => {
            if (!childProcess.killed) {
              childProcess.kill('SIGKILL');
            }
          }, 5000);
        }
      };
      
      let stdoutData = '';
      let stderrData = '';
      let outputSize = 0;
      let lastProgressUpdate = Date.now();
      
      // Handle stdout
      childProcess.stdout.on('data', (data) => {
        const output = data.toString();
        outputSize += output.length;
        
        // Check output size limit
        if (outputSize > this.config.maxOutputSize) {
          childProcess.kill('SIGTERM');
          reject(new BackendError(
            'Output size exceeded maximum limit',
            ErrorTypes.EXECUTION_FAILED,
            'OUTPUT_TOO_LARGE',
            { limit: this.config.maxOutputSize, actual: outputSize }
          ));
          return;
        }
        
        stdoutData += output;
        
        // Stream output to stderr for real-time visibility
        process.stderr.write(output);
        
        // Send progress updates (throttled)
        const now = Date.now();
        if (now - lastProgressUpdate > 1000) { // Update every second
          progressTracker.reportMessage(output.trim(), 'stdout');
          lastProgressUpdate = now;
          
          // Update session progress
          const elapsedSeconds = Math.floor((now - startTime) / 1000);
          const estimatedProgress = Math.min(25 + (elapsedSeconds * 2), 90); // Cap at 90%
          this.updateSessionStatus(request.sessionId, {
            progress: estimatedProgress
          });
        }
      });
      
      // Handle stderr
      childProcess.stderr.on('data', (data) => {
        const output = data.toString();
        stderrData += output;
        
        // Stream to stderr
        process.stderr.write(output);
        
        // Report warnings
        if (output.toLowerCase().includes('warning') || output.toLowerCase().includes('error')) {
          progressTracker.reportMessage(output.trim(), 'stderr');
        }
      });
      
      // Handle process completion
      childProcess.on('close', (code) => {
        const executionTime = Date.now() - startTime;
        
        this.log('info', `Aider process exited`, {
          code,
          executionTime,
          outputSize: stdoutData.length
        });
        
        // Parse file changes from output
        const changes = FileChangeParser.parseChanges(stdoutData + stderrData, workingDir);
        const diffStats = FileChangeParser.extractDiffStats(stdoutData + stderrData);
        
        if (code === 0) {
          resolve({
            success: true,
            sessionId: request.sessionId,
            output: stdoutData,
            changes,
            metrics: {
              executionTime,
              filesModified: changes.length,
              linesChanged: diffStats.insertions + diffStats.deletions,
              tokensUsed: TokenEstimator.estimate(request.task + stdoutData),
              exitCode: code
            },
            metadata: {
              command,
              workingDirectory: workingDir,
              aiderVersion: this.aiderVersion
            }
          });
        } else {
          reject(new BackendError(
            `Aider process exited with code ${code}`,
            ErrorTypes.EXECUTION_FAILED,
            'AIDER_PROCESS_FAILED',
            {
              exitCode: code,
              stdout: stdoutData.substring(0, 1000),
              stderr: stderrData.substring(0, 1000)
            }
          ));
        }
      });
      
      // Handle process errors
      childProcess.on('error', (error) => {
        this.log('error', 'Failed to spawn aider process', { error: error.message });
        reject(new BackendError(
          `Failed to spawn aider process: ${error.message}`,
          ErrorTypes.EXECUTION_FAILED,
          'AIDER_SPAWN_FAILED',
          { originalError: error }
        ));
      });
      
      // Set timeout
      const timeout = request.options?.timeout || this.config.timeout;
      setTimeout(() => {
        if (!childProcess.killed) {
          this.log('warn', 'Aider execution timed out', { timeout });
          childProcess.kill('SIGTERM');
          reject(new BackendError(
            `Aider execution timed out after ${timeout}ms`,
            ErrorTypes.TIMEOUT,
            'AIDER_TIMEOUT',
            { timeout }
          ));
        }
      }, timeout);
    });
  }
}

export default AiderBackend;