#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const version = process.env.GITHUB_REF?.replace('refs/tags/v', '') || process.argv[2];
if (!version) {
  console.error('Error: Version not provided');
  console.error('Usage: GITHUB_REF=refs/tags/v1.0.0 node prepare-packages.js');
  console.error('   or: node prepare-packages.js 1.0.0');
  process.exit(1);
}

console.log(`🚀 Preparing packages for version ${version}`);

// Define platform structures
const platforms = [
  'darwin-x64',
  'darwin-arm64',
  'linux-x64',
  'linux-x64-musl',
  'linux-arm64',
  'linux-arm64-musl',
  'win32-x64'
];

// Prepare platform packages
platforms.forEach(platform => {
  const sourceDir = path.join(__dirname, '..', 'platforms', platform);
  const targetDir = path.join(__dirname, '..', '..', 'npm-publish', platform);
  
  // Create directory
  fs.mkdirSync(targetDir, { recursive: true });
  
  // Read template package.json
  const templatePath = path.join(sourceDir, 'package.json');
  const packageJson = JSON.parse(fs.readFileSync(templatePath, 'utf8'));
  
  // Update version
  packageJson.version = version;
  
  // Write to target directory
  fs.writeFileSync(
    path.join(targetDir, 'package.json'),
    JSON.stringify(packageJson, null, 2) + '\n'
  );
  
  console.log(`✓ Prepared @cometix/ccline-${platform} v${version}`);
});

// Prepare main package
const mainSource = path.join(__dirname, '..', 'main');
const mainTarget = path.join(__dirname, '..', '..', 'npm-publish', 'main');

// Copy main package files
fs.cpSync(mainSource, mainTarget, { recursive: true });

// Update main package.json
const mainPackageJsonPath = path.join(mainTarget, 'package.json');
const mainPackageJson = JSON.parse(fs.readFileSync(mainPackageJsonPath, 'utf8'));

mainPackageJson.version = version;

// Update optionalDependencies versions
if (mainPackageJson.optionalDependencies) {
  Object.keys(mainPackageJson.optionalDependencies).forEach(dep => {
    if (dep.startsWith('@cometix/ccline-')) {
      mainPackageJson.optionalDependencies[dep] = version;
    }
  });
}

fs.writeFileSync(
  mainPackageJsonPath,
  JSON.stringify(mainPackageJson, null, 2) + '\n'
);

console.log(`✓ Prepared @cometix/ccline v${version}`);
console.log(`\n🎉 All packages prepared for version ${version}`);
console.log('\nNext steps:');
console.log('1. Copy binaries to platform directories');
console.log('2. Publish platform packages first');
console.log('3. Publish main package last');