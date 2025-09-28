#!/usr/bin/env node

/**
 * Simple Mermaid Diagram Testing Script using mmdc
 * 
 * Usage:
 *   # Test from stdin:
 *   echo "graph TD\n  A --> B" | node test-mermaid-mmdc.js
 *   
 *   # Test from file:
 *   node test-mermaid-mmdc.js diagram.mmd
 *   
 *   # Test with verbose output:
 *   node test-mermaid-mmdc.js --verbose diagram.mmd
 */

import { readFileSync, writeFileSync, unlinkSync } from 'fs';
import { exec } from 'child_process';
import { promisify } from 'util';
import chalk from 'chalk';
import { tmpdir } from 'os';
import { join } from 'path';

const execPromise = promisify(exec);

// Parse command line arguments
const args = process.argv.slice(2);
const verbose = args.includes('--verbose') || args.includes('-v');
const filePath = args.find(arg => !arg.startsWith('-'));

// Helper function to extract diagram content
function extractDiagramContent(input) {
  // Remove markdown code block markers if present
  const mermaidBlockRegex = /```mermaid\s*\n?([\s\S]*?)\n?```/;
  const match = input.match(mermaidBlockRegex);
  if (match) {
    return match[1].trim();
  }
  
  // If no code block, assume the entire input is the diagram
  return input.trim();
}

// Function to test a diagram
async function testDiagram(diagramContent, source = 'input') {
  console.log(chalk.blue(`\n━━━ Testing ${source} ━━━`));
  
  if (verbose) {
    console.log(chalk.gray('Input diagram:'));
    console.log(chalk.gray(diagramContent));
    console.log();
  }
  
  // Create temporary files
  const tempInput = join(tmpdir(), `mermaid-test-${Date.now()}.mmd`);
  const tempOutput = join(tmpdir(), `mermaid-test-${Date.now()}.svg`);
  
  try {
    // Write diagram to temp file
    writeFileSync(tempInput, diagramContent, 'utf8');
    
    // Try to parse the diagram using mmdc (mermaid CLI)
    const mmdcPath = join(process.cwd(), 'node_modules/.bin/mmdc');
    const command = `"${mmdcPath}" -i "${tempInput}" -o "${tempOutput}" -q`; // -q for quiet mode
    
    try {
      await execPromise(command);
      console.log(chalk.green('✓ Diagram is valid!'));
      
      if (verbose) {
        console.log(chalk.gray('\nDiagram successfully parsed'));
        
        // Try to identify diagram type from the content
        const firstLine = diagramContent.split('\n')[0].trim().toLowerCase();
        let diagramType = 'unknown';
        
        if (firstLine.includes('graph') || firstLine.includes('flowchart')) {
          diagramType = 'flowchart';
        } else if (firstLine.includes('sequencediagram')) {
          diagramType = 'sequence';
        } else if (firstLine.includes('classdiagram')) {
          diagramType = 'class';
        } else if (firstLine.includes('statediagram')) {
          diagramType = 'state';
        } else if (firstLine.includes('erdiagram')) {
          diagramType = 'entity-relationship';
        } else if (firstLine.includes('journey')) {
          diagramType = 'user journey';
        } else if (firstLine.includes('gantt')) {
          diagramType = 'gantt';
        } else if (firstLine.includes('pie')) {
          diagramType = 'pie chart';
        } else if (firstLine.includes('gitgraph')) {
          diagramType = 'git graph';
        } else if (firstLine.includes('mindmap')) {
          diagramType = 'mindmap';
        } else if (firstLine.includes('timeline')) {
          diagramType = 'timeline';
        }
        
        console.log(chalk.gray(`Detected type: ${diagramType}`));
      }
      
      return true;
    } catch (error) {
      // Extract error message from mmdc output
      const errorMessage = error.stderr || error.message || 'Unknown error';
      console.log(chalk.red(`✗ ${errorMessage}`));
      
      if (verbose && error.stack) {
        console.log(chalk.gray('\nStack trace:'));
        console.log(chalk.gray(error.stack));
      }
      
      return false;
    }
  } finally {
    // Clean up temp files
    try {
      unlinkSync(tempInput);
    } catch (e) {
      // Ignore cleanup errors
    }
    try {
      unlinkSync(tempOutput);
    } catch (e) {
      // Ignore cleanup errors  
    }
  }
}

// Main function
async function main() {
  let input;
  let source;
  
  if (filePath) {
    // Read from file
    try {
      input = readFileSync(filePath, 'utf8');
      source = `file: ${filePath}`;
    } catch (error) {
      console.error(chalk.red(`Error reading file: ${error.message}`));
      process.exit(1);
    }
  } else if (!process.stdin.isTTY) {
    // Read from stdin
    input = '';
    process.stdin.setEncoding('utf8');
    
    for await (const chunk of process.stdin) {
      input += chunk;
    }
    source = 'stdin';
  } else {
    // No input provided
    console.log(chalk.yellow('Mermaid Diagram Tester (using mmdc CLI)'));
    console.log(chalk.gray('\nUsage:'));
    console.log(chalk.gray('  # Test from stdin:'));
    console.log(chalk.gray('  echo "graph TD\\n  A --> B" | node test-mermaid-mmdc.js'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('  # Test from file:'));
    console.log(chalk.gray('  node test-mermaid-mmdc.js diagram.mmd'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('  # Test with verbose output:'));
    console.log(chalk.gray('  node test-mermaid-mmdc.js --verbose diagram.mmd'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('Options:'));
    console.log(chalk.gray('  --verbose, -v    Show detailed output'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('Supports all Mermaid diagram types:'));
    console.log(chalk.gray('  flowchart, graph, sequenceDiagram, classDiagram, stateDiagram'));
    console.log(chalk.gray('  erDiagram, journey, gantt, pie, gitGraph, mindmap, timeline, etc.'));
    process.exit(0);
  }
  
  // Extract and test the diagram
  const diagramContent = extractDiagramContent(input);
  const isValid = await testDiagram(diagramContent, source);
  
  // Exit with appropriate code
  process.exit(isValid ? 0 : 1);
}

// Handle errors
process.on('unhandledRejection', (error) => {
  console.error(chalk.red('Unhandled error:'), error);
  process.exit(1);
});

// Run the script
main().catch(error => {
  console.error(chalk.red('Fatal error:'), error);
  process.exit(1);
});