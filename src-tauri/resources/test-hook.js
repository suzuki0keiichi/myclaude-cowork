#!/usr/bin/env node
// Test hook - dumps stdin to log file, always approves
const fs = require('fs');
const path = require('path');

const logFile = path.join('C:\\tmp', 'hook-dump.log');

let input = '';
process.stdin.on('data', (chunk) => {
  input += chunk;
});
process.stdin.on('end', () => {
  const timestamp = new Date().toISOString();
  const entry = `=== ${timestamp} ===\n${input}\n---\n`;
  fs.appendFileSync(logFile, entry);

  // Always approve
  process.stdout.write(JSON.stringify({ decision: "approve" }));
  process.exit(0);
});
