/**
 * Tests to ensure all required files are listed in package.json "files" array.
 * This prevents publishing broken packages where imports fail due to missing files.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { readFileSync, existsSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const packageRoot = resolve(__dirname, '../..');

/**
 * Check if a file path is covered by any directory entry in the files array.
 * For example, "storage/JsonChatStorage.js" is covered by "storage/".
 */
function isFileCoveredByDirectory(filePath, filesArray) {
    const parts = filePath.split('/');
    if (parts.length > 1) {
        // Check if any parent directory is in the files array
        const dir = parts[0] + '/';
        return filesArray.includes(dir);
    }
    return false;
}

describe('Package Files Validation', () => {
    it('should include all locally imported JS files in the files array', () => {
        const packageJsonPath = join(packageRoot, 'package.json');
        const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf-8'));
        const filesArray = packageJson.files || [];

        // Get all top-level JS files that are imported by files in the files array
        const jsFilesInArray = filesArray.filter(f => f.endsWith('.js') && !f.includes('/'));

        // Read each JS file and find local imports
        const missingFiles = [];

        for (const jsFile of jsFilesInArray) {
            const filePath = join(packageRoot, jsFile);
            if (!existsSync(filePath)) continue;

            const content = readFileSync(filePath, 'utf-8');

            // Match ES6 imports like: import { X } from './SomeFile.js'
            const importRegex = /from\s+['"]\.\/([^'"]+\.js)['"]/g;
            let match;

            while ((match = importRegex.exec(content)) !== null) {
                const importedFile = match[1];
                // Check if this local import is in the files array (directly or via directory)
                const isDirectlyIncluded = filesArray.includes(importedFile);
                const isCoveredByDir = isFileCoveredByDirectory(importedFile, filesArray);

                if (!isDirectlyIncluded && !isCoveredByDir) {
                    // Verify the file actually exists
                    if (existsSync(join(packageRoot, importedFile))) {
                        missingFiles.push({
                            importedBy: jsFile,
                            missingFile: importedFile
                        });
                    }
                }
            }
        }

        if (missingFiles.length > 0) {
            const errorMsg = missingFiles
                .map(m => `  - "${m.missingFile}" (imported by ${m.importedBy})`)
                .join('\n');
            assert.fail(
                `The following files are imported but not listed in package.json "files" array:\n${errorMsg}\n` +
                `Add them to the "files" array to ensure they are included in the published package.`
            );
        }
    });

    it('should have all files in the files array actually exist', () => {
        const packageJsonPath = join(packageRoot, 'package.json');
        const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf-8'));
        const filesArray = packageJson.files || [];

        const missingFiles = [];

        for (const file of filesArray) {
            const filePath = join(packageRoot, file);
            if (!existsSync(filePath)) {
                missingFiles.push(file);
            }
        }

        if (missingFiles.length > 0) {
            assert.fail(
                `The following files are listed in package.json "files" but do not exist:\n` +
                missingFiles.map(f => `  - ${f}`).join('\n')
            );
        }
    });

    it('should include ChatSessionManager.js which is required by webServer.js', () => {
        // Explicit test for the specific bug that was found
        const packageJsonPath = join(packageRoot, 'package.json');
        const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf-8'));
        const filesArray = packageJson.files || [];

        assert.ok(
            filesArray.includes('ChatSessionManager.js'),
            'ChatSessionManager.js must be in the files array - it is required by webServer.js for --web mode'
        );

        // Also verify webServer.js is included
        assert.ok(
            filesArray.includes('webServer.js'),
            'webServer.js must be in the files array - it provides the --web functionality'
        );
    });
});
