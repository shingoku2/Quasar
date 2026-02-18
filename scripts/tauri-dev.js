#!/usr/bin/env node
/**
 * Run Tauri dev with CARGO_TARGET_DIR set to this project's absolute path.
 * Overrides any global CARGO_TARGET_DIR (e.g. from a renamed/moved project).
 */
import path from 'path';
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '..');
const targetDir = path.join(projectRoot, 'src-tauri', 'target');

const env = { ...process.env, CARGO_TARGET_DIR: targetDir };
const args = ['tauri', ...process.argv.slice(2)];
// Use npx.cmd on Windows so we can pass args safely without shell (avoids DEP0190)
const cmd = process.platform === 'win32' ? 'npx.cmd' : 'npx';
const child = spawn(cmd, args, {
  stdio: 'inherit',
  env,
  cwd: projectRoot,
});

child.on('close', (code) => process.exit(code ?? 1));
child.on('error', (err) => {
  console.error(err);
  process.exit(1);
});
