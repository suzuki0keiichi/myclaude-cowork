#!/usr/bin/env node
// Cowork PreToolUse hook
// Sends tool approval requests to the Cowork approval server via HTTP.
// Falls back to auto-approve if the server is unreachable.

const http = require('http');

// Get approval port from env var (only set by Cowork app)
const port = process.env.COWORK_APPROVAL_PORT || null;

// Write output and exit safely (wait for stdout flush)
function writeAndExit(output) {
  if (output) {
    process.stdout.write(JSON.stringify(output), () => {
      process.exit(0);
    });
  } else {
    process.exit(0);
  }
}

let input = '';
process.stdin.on('data', (chunk) => {
  input += chunk;
});
process.stdin.on('end', () => {
  if (!port) {
    // No server port configured, auto-approve
    process.stderr.write('cowork-hook: no approval port found (env or file)\n');
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
          writeAndExit({
            hookSpecificOutput: {
              hookEventName: "PreToolUse",
              permissionDecision: "allow"
            }
          });
        } else {
          writeAndExit({
            hookSpecificOutput: {
              hookEventName: "PreToolUse",
              permissionDecision: "deny",
              permissionDecisionReason: "ユーザーが操作を拒否しました"
            }
          });
        }
      } catch (e) {
        process.stderr.write('cowork-hook: JSON parse error: ' + e.message + '\n');
        process.exit(0);
      }
    });
  });

  req.on('error', (e) => {
    // Server unreachable, auto-approve (exit 0 with no output = allow)
    process.stderr.write('cowork-hook: connection error: ' + e.message + '\n');
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
