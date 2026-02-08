use axum::{
    Router,
    extract::{Query, State as AxumState},
    response::Html,
    routing::get,
};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

#[derive(serde::Deserialize)]
struct CallbackParams {
    code: Option<String>,
    error: Option<String>,
}

struct CallbackState {
    tx: Mutex<Option<oneshot::Sender<Result<String, String>>>>,
}

async fn handle_callback(
    Query(params): Query<CallbackParams>,
    AxumState(state): AxumState<Arc<CallbackState>>,
) -> Html<&'static str> {
    let result = if let Some(code) = params.code {
        Ok(code)
    } else if let Some(error) = params.error {
        Err(format!("認証が拒否されました: {}", error))
    } else {
        Err("認証コードを取得できませんでした".to_string())
    };

    let is_ok = result.is_ok();
    if let Some(tx) = state.tx.lock().await.take() {
        let _ = tx.send(result);
    }

    if is_ok {
        Html(concat!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>認証完了</title>",
            "<style>body{font-family:system-ui,sans-serif;display:flex;justify-content:center;",
            "align-items:center;min-height:100vh;margin:0;background:#f8f9fa;}",
            ".card{text-align:center;padding:48px;background:white;border-radius:12px;",
            "box-shadow:0 2px 12px rgba(0,0,0,0.1);}",
            "h1{color:#22c55e;margin-bottom:8px;}p{color:#666;}</style></head>",
            "<body><div class=\"card\"><h1>認証が完了しました</h1>",
            "<p>このタブを閉じて、アプリに戻ってください。</p></div></body></html>",
        ))
    } else {
        Html(concat!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>認証エラー</title>",
            "<style>body{font-family:system-ui,sans-serif;display:flex;justify-content:center;",
            "align-items:center;min-height:100vh;margin:0;background:#f8f9fa;}",
            ".card{text-align:center;padding:48px;background:white;border-radius:12px;",
            "box-shadow:0 2px 12px rgba(0,0,0,0.1);}",
            "h1{color:#ef4444;margin-bottom:8px;}p{color:#666;}</style></head>",
            "<body><div class=\"card\"><h1>認証に失敗しました</h1>",
            "<p>アプリに戻って、もう一度お試しください。</p></div></body></html>",
        ))
    }
}

/// Start a temporary local HTTP server to receive an OAuth callback.
/// Returns (port, receiver that yields the auth code or an error).
pub async fn wait_for_oauth_callback(
) -> Result<(u16, oneshot::Receiver<Result<String, String>>), String> {
    let (tx, rx) = oneshot::channel();
    let state = Arc::new(CallbackState {
        tx: Mutex::new(Some(tx)),
    });

    let app = Router::new()
        .route("/callback", get(handle_callback))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("認証用サーバーの起動に失敗しました: {}", e))?;

    let port = listener
        .local_addr()
        .map_err(|e| format!("ポート取得エラー: {}", e))?
        .port();

    log::info!("OAuth callback server started on port {}", port);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("OAuth callback server error: {}", e);
        }
    });

    Ok((port, rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_starts_and_returns_port() {
        let (port, _rx) = wait_for_oauth_callback().await.unwrap();
        assert!(port > 0);
    }

    #[tokio::test]
    async fn test_callback_with_code() {
        let (port, rx) = wait_for_oauth_callback().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/callback?code=test_auth_code", port))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.text().await.unwrap();
        assert!(body.contains("認証が完了しました"));

        let result = rx.await.unwrap();
        assert_eq!(result.unwrap(), "test_auth_code");
    }

    #[tokio::test]
    async fn test_callback_with_error() {
        let (port, rx) = wait_for_oauth_callback().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/callback?error=access_denied", port))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.text().await.unwrap();
        assert!(body.contains("認証に失敗しました"));

        let result = rx.await.unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("access_denied"));
    }

    #[tokio::test]
    async fn test_callback_with_no_params() {
        let (port, rx) = wait_for_oauth_callback().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/callback", port))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.text().await.unwrap();
        assert!(body.contains("認証に失敗しました"));

        let result = rx.await.unwrap();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_second_callback_ignored() {
        let (port, rx) = wait_for_oauth_callback().await.unwrap();

        let client = reqwest::Client::new();
        // First callback should succeed
        client
            .get(format!("http://127.0.0.1:{}/callback?code=first_code", port))
            .send()
            .await
            .unwrap();

        // Second callback should not panic (tx already taken)
        let resp2 = client
            .get(format!("http://127.0.0.1:{}/callback?code=second_code", port))
            .send()
            .await
            .unwrap();
        assert_eq!(resp2.status(), 200);

        let result = rx.await.unwrap();
        assert_eq!(result.unwrap(), "first_code");
    }
}
