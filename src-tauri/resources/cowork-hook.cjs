#!/usr/bin/env node
// Cowork PreToolUse hook
// Sends tool approval requests to the Cowork approval server via HTTP.
// Falls back to auto-approve if the server is unreachable.

const http = require('http');

const port = process.env.COWORK_APPROVAL_PORT;

let input = '';
process.stdin.on('data', (chunk) => {
  input += chunk;
});
process.stdin.on('end', () => {
  if (!port) {
    // No server port configured, auto-approve
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
          // Approved: use hookSpecificOutput format
          process.stdout.write(JSON.stringify({
            hookSpecificOutput: {
              hookEventName: "PreToolUse",
              permissionDecision: "allow"
            }
          }));
        } else {
          // Denied
          process.stdout.write(JSON.stringify({
            hookSpecificOutput: {
              hookEventName: "PreToolUse",
              permissionDecision: "deny",
              permissionDecisionReason: "ユーザーが操作を拒否しました"
            }
          }));
        }
      } catch (e) {
        // Parse error, auto-approve
      }
      process.exit(0);
    });
  });

  req.on('error', () => {
    // Server unreachable, auto-approve (exit 0 with no output = allow)
    process.exit(0);
  });

  // Timeout after 130 seconds (slightly longer than server's 120s timeout)
  req.setTimeout(130000, () => {
    req.destroy();
    process.exit(0);
  });

  req.write(postData);
  req.end();
});
