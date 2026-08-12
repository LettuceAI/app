#!/usr/bin/env node
/**
 * Syncs version from package.json to tauri.conf.json, Cargo.toml, and utils.rs
 * Run this before builds to ensure versions stay in sync.
 */

import { readFileSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = join(__dirname, '..');

const packageJsonPath = join(rootDir, 'package.json');
const tauriConfPath = join(rootDir, 'src-tauri', 'tauri.conf.json');
const cargoTomlPath = join(rootDir, 'src-tauri', 'Cargo.toml');
const utilsRsPath = join(rootDir, 'src-tauri', 'src', 'utils.rs');

const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf-8'));
const version = packageJson.version;

console.log(`Target version: ${version}`);

const tauriConf = JSON.parse(readFileSync(tauriConfPath, 'utf-8'));
if (tauriConf.version !== version) {
    console.log(`  tauri.conf.json: ${tauriConf.version} -> ${version}`);
    tauriConf.version = version;
    writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
} else {
    console.log(`  tauri.conf.json: already ${version}`);
}

let cargoToml = readFileSync(cargoTomlPath, 'utf-8');
const cargoVersionRegex = /^version = ".*"$/m;
const cargoMatch = cargoToml.match(cargoVersionRegex);
if (cargoMatch && cargoMatch[0] !== `version = "${version}"`) {
    console.log(`  Cargo.toml: ${cargoMatch[0]} -> version = "${version}"`);
    cargoToml = cargoToml.replace(cargoVersionRegex, `version = "${version}"`);
    writeFileSync(cargoTomlPath, cargoToml);
} else {
    console.log(`  Cargo.toml: already ${version}`);
}

let utilsRs = readFileSync(utilsRsPath, 'utf-8');
const utilsVersionRegex = /"[\d\.a-z-]+"/;
const utilsMatch = utilsRs.match(utilsVersionRegex);
const targetUtilsVersion = `"${version}"`;
if (utilsMatch && utilsMatch[0] !== targetUtilsVersion) {
    console.log(`  utils.rs: ${utilsMatch[0]} -> ${targetUtilsVersion}`);
    utilsRs = utilsRs.replace(utilsVersionRegex, targetUtilsVersion);
    writeFileSync(utilsRsPath, utilsRs);
} else {
    console.log(`  utils.rs: already ${targetUtilsVersion}`);
}

console.log('Version sync complete!');
