#!/bin/bash
# Script to migrate from TypeScript to JavaScript version

echo "Migrating from TypeScript to JavaScript version..."

# Check if TypeScript files exist
if [ ! -f "src/index.ts" ]; then
  echo "TypeScript files not found. You may already be using the JavaScript version."
  exit 0
fi

# Backup TypeScript files
echo "Creating backup of TypeScript files..."
mkdir -p backup/src
cp -r src/*.ts backup/src/
cp tsconfig.json backup/ 2>/dev/null

# Remove TypeScript files and build directory
echo "Removing TypeScript files and build directory..."
rm -f src/*.ts
rm -f tsconfig.json
rm -rf build

# Make JavaScript files executable
echo "Making JavaScript files executable..."
chmod +x src/index.js

# Update package.json
echo "Updating package.json..."
# This is a simple sed command to update package.json
# For a more robust solution, consider using a JSON parser
sed -i.bak 's/"main": "build\/index.js"/"main": "src\/index.js"/g' package.json
sed -i.bak 's/"bin": {[^}]*}/"bin": {\n    "probe-agent-mcp": "src\/index.js"\n  }/g' package.json
sed -i.bak 's/"files": \[[^]]*\]/"files": [\n    "src\/**\/*.js"\n  ]/g' package.json
sed -i.bak 's/"scripts": {[^}]*}/"scripts": {\n    "prepare": "chmod +x src\/index.js",\n    "start": "node src\/index.js",\n    "dev": "node src\/index.js"\n  }/g' package.json
sed -i.bak '/"devDependencies": {/,/}/d' package.json

# Clean up backup files
rm -f package.json.bak

echo "Migration complete!"
echo "You may need to reinstall dependencies with 'npm install'"
echo "Start the server with 'npm start' or 'node src/index.js'"