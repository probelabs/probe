#!/usr/bin/env node

/**
 * Standalone Mermaid Diagram Testing Script
 * 
 * Usage:
 *   # Test from stdin:
 *   echo "graph TD\n  A --> B" | node test-mermaid.js
 *   
 *   # Test from file:
 *   node test-mermaid.js diagram.mmd
 *   
 *   # Test with verbose output:
 *   node test-mermaid.js --verbose diagram.mmd
 *   
 *   # Use strict parser (limited diagram types):
 *   node test-mermaid.js --strict diagram.mmd
 * 
 * Note: The @mermaid-js/parser only supports: pie, packet, gitGraph, architecture, info, radar, treemap
 *       By default, this script uses a regex-based validator for broader diagram type support.
 */

import { readFileSync, writeFileSync } from 'fs';
import { parse } from '@mermaid-js/parser';
import { validateAndFixMermaidResponse } from './src/agent/schemaUtils.js';
import chalk from 'chalk';

// Parse command line arguments
const args = process.argv.slice(2);

// Check for unknown options and typos
const knownOptions = ['--verbose', '-v', '--strict', '-s', '--fix', '-f'];
const unknownOptions = args.filter(arg => arg.startsWith('-') && !knownOptions.includes(arg));
if (unknownOptions.length > 0) {
  console.error(chalk.red(`Unknown option(s): ${unknownOptions.join(', ')}`));
  if (unknownOptions.some(opt => opt.includes('stict'))) {
    console.error(chalk.yellow('Did you mean --strict instead of --stict?'));
  }
  process.exit(1);
}

const verbose = args.includes('--verbose') || args.includes('-v');
const strict = args.includes('--strict') || args.includes('-s');
const autoFix = args.includes('--fix') || args.includes('-f');
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

// Helper function to format error messages
function formatError(error) {
  if (error.hash) {
    const { text, token, line, loc, expected } = error.hash;
    let message = chalk.red(`✗ Parse Error at line ${line || 'unknown'}`);
    
    if (loc && loc.first_line !== undefined) {
      message += chalk.red(` (${loc.first_line}:${loc.first_column})`);
    }
    
    if (text) {
      message += chalk.yellow(`\n  Found: "${text}"`);
    }
    
    if (expected && expected.length > 0) {
      message += chalk.cyan(`\n  Expected: ${expected.join(', ')}`);
    }
    
    if (token) {
      message += chalk.gray(`\n  Token: ${token}`);
    }
    
    return message;
  }
  
  return chalk.red(`✗ ${error.message || error}`);
}

// Regex-based validator for common Mermaid diagrams
function validateWithRegex(diagramContent) {
  const lines = diagramContent.split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('%%'));
  if (lines.length === 0) {
    throw new Error('Empty diagram');
  }
  
  const firstLine = lines[0].toLowerCase();
  
  // Check for valid diagram types
  const validTypes = [
    'graph', 'flowchart', 'sequencediagram', 'classDiagram', 'stateDiagram',
    'erDiagram', 'journey', 'gantt', 'pie', 'gitGraph', 'mindmap',
    'timeline', 'quadrantChart', 'requirementDiagram', 'c4Context',
    'architecture', 'packet', 'info', 'radar', 'treemap'
  ];
  
  const hasValidType = validTypes.some(type => 
    firstLine.startsWith(type.toLowerCase()) || 
    firstLine.includes(type.toLowerCase() + ' ')
  );
  
  if (!hasValidType && !firstLine.startsWith('%%{')) {
    throw new Error(`Unknown diagram type. First line: "${lines[0]}"`);
  }
  
  // Basic syntax checks
  const errors = [];
  
  // Check for common syntax errors
  lines.forEach((line, index) => {
    // Check for unmatched brackets in node definitions
    const openBrackets = (line.match(/[\[({]/g) || []).length;
    const closeBrackets = (line.match(/[\])}]/g) || []).length;
    if (openBrackets !== closeBrackets && !line.includes('-->') && !line.includes('---')) {
      errors.push(`Line ${index + 1}: Unmatched brackets`);
    }
    
    // Check for invalid single quotes in node labels (GitHub issue)
    if (line.match(/\{[^{}]*'[^{}]*\}/) || line.match(/\[[^\[\]]*'[^\[\]]*\]/)) {
      errors.push(`Line ${index + 1}: Single quotes in node labels may cause GitHub rendering issues`);
    }
    
    // Check for unclosed strings
    const quotes = line.split('"').length - 1;
    if (quotes % 2 !== 0) {
      errors.push(`Line ${index + 1}: Unclosed string`);
    }
  });
  
  if (errors.length > 0) {
    throw new Error('Validation errors:\n  ' + errors.join('\n  '));
  }
  
  return {
    type: firstLine.split(/\s+/)[0],
    lineCount: lines.length
  };
}

// Function to test a diagram
async function testDiagram(diagramContent, source = 'input') {
  console.log(chalk.blue(`\n━━━ Testing ${source} ━━━`));
  
  if (verbose) {
    console.log(chalk.gray('Input diagram:'));
    console.log(chalk.gray(diagramContent));
    console.log();
  }
  
  try {
    if (strict) {
      // Use the strict @mermaid-js/parser (limited diagram types)
      console.log(chalk.gray('Using strict parser (@mermaid-js/parser)...'));
      await parse(diagramContent);
      console.log(chalk.green('✓ Diagram is valid (strict parser)!'));
    } else {
      // Use regex-based validation for broader support
      console.log(chalk.gray('Using regex-based validator...'));
      const result = validateWithRegex(diagramContent);
      console.log(chalk.green('✓ Diagram is valid!'));
      
      if (verbose) {
        console.log(chalk.gray(`\nDiagram type: ${result.type}`));
        console.log(chalk.gray(`Lines of content: ${result.lineCount}`));
      }
    }
    
    if (verbose) {
      // Additional analysis
      const lines = diagramContent.split('\n').filter(l => l.trim() && !l.trim().startsWith('%%'));
      
      // Count specific elements
      const nodeCount = (diagramContent.match(/\[[^\]]*\]/g) || []).length;
      const arrowCount = (diagramContent.match(/-->/g) || []).length + 
                        (diagramContent.match(/->>/g) || []).length +
                        (diagramContent.match(/---/g) || []).length;
      
      if (nodeCount > 0) console.log(chalk.gray(`Nodes found: ${nodeCount}`));
      if (arrowCount > 0) console.log(chalk.gray(`Connections found: ${arrowCount}`));
      
      // Check for potential issues
      const warnings = [];
      if (diagramContent.includes("'")) {
        warnings.push("Contains single quotes - may cause GitHub rendering issues");
      }
      if (diagramContent.includes("&apos;")) {
        warnings.push("Contains &apos; entities - consider using &#39; instead");
      }
      
      if (warnings.length > 0) {
        console.log(chalk.yellow('\nWarnings:'));
        warnings.forEach(w => console.log(chalk.yellow(`  ⚠ ${w}`)));
      }
    }
    
    return true;
  } catch (error) {
    console.log(formatError(error));
    
    if (verbose && error.stack) {
      console.log(chalk.gray('\nStack trace:'));
      console.log(chalk.gray(error.stack));
    }
    
    return false;
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
    console.log(chalk.yellow('Mermaid Diagram Tester'));
    console.log(chalk.gray('\nUsage:'));
    console.log(chalk.gray('  # Test from stdin:'));
    console.log(chalk.gray('  echo "graph TD\\n  A --> B" | node test-mermaid.js'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('  # Test from file:'));
    console.log(chalk.gray('  node test-mermaid.js diagram.mmd'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('  # Test with verbose output:'));
    console.log(chalk.gray('  node test-mermaid.js --verbose diagram.mmd'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('  # Use strict parser:'));
    console.log(chalk.gray('  node test-mermaid.js --strict diagram.mmd'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('  # Auto-fix problematic diagrams:'));
    console.log(chalk.gray('  node test-mermaid.js --fix diagram.mmd'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('Options:'));
    console.log(chalk.gray('  --verbose, -v    Show detailed output'));
    console.log(chalk.gray('  --strict, -s     Use @mermaid-js/parser (limited to: pie, gitGraph, etc.)'));
    console.log(chalk.gray('  --fix, -f        Auto-fix problematic diagrams (quotes, entities, etc.)'));
    console.log(chalk.gray(''));
    console.log(chalk.gray('Supported diagram types (regex validator):'));
    console.log(chalk.gray('  graph, flowchart, sequenceDiagram, classDiagram, stateDiagram'));
    console.log(chalk.gray('  erDiagram, journey, gantt, pie, gitGraph, mindmap, timeline'));
    console.log(chalk.gray('  quadrantChart, requirementDiagram, c4Context, and more'));
    process.exit(0);
  }
  
  // Extract and test the diagram
  const diagramContent = extractDiagramContent(input);
  
  if (autoFix) {
    // Use the auto-fix functionality
    console.log(chalk.blue(`\n━━━ Auto-fixing ${source} ━━━`));
    
    try {
      const wrappedContent = `\`\`\`mermaid\n${diagramContent}\n\`\`\``;
      const result = await validateAndFixMermaidResponse(wrappedContent, { 
        autoFix: true, 
        debug: verbose 
      });
      
      if (result.wasFixed) {
        console.log(chalk.green('✓ Diagram was fixed!'));
        
        // Extract the fixed diagram content
        const fixedContent = extractDiagramContent(result.fixedResponse);
        
        if (filePath) {
          // Save the fixed content back to the file
          const outputPath = filePath.replace(/\.mmd$/, '.fixed.mmd');
          writeFileSync(outputPath, fixedContent, 'utf8');
          console.log(chalk.green(`✓ Fixed diagram saved to: ${outputPath}`));
        } else {
          // Output the fixed content to stdout
          console.log(chalk.blue('\n━━━ Fixed diagram ━━━'));
          console.log(fixedContent);
        }
        
        // Validate the fixed diagram
        const isValid = await testDiagram(fixedContent, 'fixed diagram');
        process.exit(isValid ? 0 : 1);
      } else {
        console.log(chalk.yellow('ℹ Diagram did not need fixing'));
        const isValid = await testDiagram(diagramContent, source);
        process.exit(isValid ? 0 : 1);
      }
    } catch (error) {
      console.error(chalk.red(`Auto-fix failed: ${error.message}`));
      process.exit(1);
    }
  } else {
    const isValid = await testDiagram(diagramContent, source);
    
    // Exit with appropriate code
    process.exit(isValid ? 0 : 1);
  }
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