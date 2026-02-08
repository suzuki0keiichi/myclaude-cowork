#!/usr/bin/env node
// E2E test hook: sends approval request to HTTP server and waits for response
const http = require('http');

const port = process.env.COWORK_APPROVAL_PORT;

let input = '';
process.stdin.on('data', (chunk) => {
  input += chunk;
});
process.stdin.on('end', () => {
  if (!port) {
    // No server configured, auto-approve
    process.stdout.write(JSON.stringify({ decision: "approve" }));
    process.exit(0);
    return;
  }

  const postData = input;
  const options = {
    hostname: '127.0.0.1',
    port: parseInt(port),
    path: '/approval',
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Content-Length': Buffer.byteLength(postData),
    },
  };

  const req = http.request(options, (res) => {
    let responseData = '';
    res.on('data', (chunk) => { responseData += chunk; });
    res.on('end', () => {
      try {
        const parsed = JSON.parse(responseData);
        if (parsed.approved) {
          // Use new hookSpecificOutput format
          process.stdout.write(JSON.stringify({
            hookSpecificOutput: {
              hookEventName: "PreToolUse",
              permissionDecision: "allow",
              permissionDecisionReason: "Approved by user"
            }
          }));
        } else {
          process.stdout.write(JSON.stringify({
            hookSpecificOutput: {
              hookEventName: "PreToolUse",
              permissionDecision: "deny",
              permissionDecisionReason: "Denied by user"
            }
          }));
        }
      } catch (e) {
        // Parse error, auto-approve
        process.stdout.write(JSON.stringify({
          hookSpecificOutput: {
            hookEventName: "PreToolUse",
            permissionDecision: "allow",
            permissionDecisionReason: "Fallback: parse error"
          }
        }));
      }
      process.exit(0);
    });
  });

  req.on('error', () => {
    // Server unreachable, auto-approve
    process.stdout.write(JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "allow",
        permissionDecisionReason: "Fallback: server unreachable"
      }
    }));
    process.exit(0);
  });

  req.write(postData);
  req.end();
});
