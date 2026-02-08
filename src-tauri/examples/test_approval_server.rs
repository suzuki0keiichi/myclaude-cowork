//! 検証 0-2: axum + oneshot チャネルによるブロッキング応答パターン
//!
//! POST /approval  — hookからの承認リクエスト受信、応答待ちでブロック
//! POST /respond   — 外部からの承認/拒否応答、ブロック解除
//!
//! テスト手順:
//!   1. cargo run --example test_approval_server
//!   2. curl -X POST http://127.0.0.1:<port>/approval -d '{"tool_name":"Bash","tool_input":{"command":"rm -rf /"}}'
//!      → ブロックされる
//!   3. 別ターミナルで: curl -X POST http://127.0.0.1:<port>/respond -d '{"approved":true}'
//!      → /approval のリクエストが返る

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::post,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};

#[derive(Clone)]
struct AppState {
    /// approval_id -> oneshot sender
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
}

async fn handle_approval(
    State(state): State<AppState>,
    body: String,
) -> Result<String, StatusCode> {
    let approval_id = uuid::Uuid::new_v4().to_string();
    println!("[approval] Received request (id={}): {}", approval_id, body);

    let (tx, rx) = oneshot::channel::<bool>();

    // Store the sender
    {
        let mut pending = state.pending.lock().await;
        pending.insert(approval_id.clone(), tx);
    }

    println!("[approval] Waiting for response (id={})...", approval_id);

    // Block until response or timeout
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        rx,
    ).await;

    match result {
        Ok(Ok(approved)) => {
            println!("[approval] Got response: approved={}", approved);
            let response = serde_json::json!({
                "approval_id": approval_id,
                "approved": approved,
            });
            Ok(response.to_string())
        }
        Ok(Err(_)) => {
            println!("[approval] Channel closed (sender dropped)");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
        Err(_) => {
            println!("[approval] Timeout after 60s, auto-approving");
            // Clean up
            let mut pending = state.pending.lock().await;
            pending.remove(&approval_id);
            let response = serde_json::json!({
                "approval_id": approval_id,
                "approved": true,
                "reason": "timeout",
            });
            Ok(response.to_string())
        }
    }
}

async fn handle_respond(
    State(state): State<AppState>,
    body: String,
) -> Result<String, StatusCode> {
    println!("[respond] Received: {}", body);

    let parsed: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let approved = parsed["approved"].as_bool().unwrap_or(true);

    // Find any pending approval and respond to it
    let tx = {
        let mut pending = state.pending.lock().await;
        let key = pending.keys().next().cloned();
        key.and_then(|k| pending.remove(&k))
    };

    if let Some(tx) = tx {
        let _ = tx.send(approved);
        Ok(format!("Responded with approved={}", approved))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[tokio::main]
async fn main() {
    let state = AppState {
        pending: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/approval", post(handle_approval))
        .route("/respond", post(handle_respond))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    println!("Approval server listening on http://{}", addr);
    println!("Test with:");
    println!("  curl -X POST http://{}/approval -d '{{\"tool_name\":\"Bash\"}}'", addr);
    println!("  curl -X POST http://{}/respond -d '{{\"approved\":true}}'", addr);

    axum::serve(listener, app).await.unwrap();
}
