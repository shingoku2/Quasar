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
const child = spawn('npx', args, {
  stdio: 'inherit',
  env,
  cwd: projectRoot,
  shell: process.platform === 'win32',
});

child.on('close', (code) => process.exit(code ?? 1));
child.on('error', (err) => {
  console.error(err);
  process.exit(1);
});
