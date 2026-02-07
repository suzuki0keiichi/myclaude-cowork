import { useRef, useEffect } from "react";
import { useClaude } from "./hooks/useClaude";
import { MessageBubble } from "./components/MessageBubble";
import { ChatInput } from "./components/ChatInput";
import { ActivityPanel } from "./components/ActivityPanel";
import { StreamingIndicator } from "./components/StreamingIndicator";
import { SetupScreen } from "./components/SetupScreen";
import "./App.css";

function App() {
  const {
    messages,
    activities,
    isLoading,
    streamingText,
    workingDir,
    error,
    sendMessage,
    changeWorkingDir,
    clearMessages,
  } = useClaude();

  const chatEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingText]);

  if (!workingDir) {
    return <SetupScreen onSetup={changeWorkingDir} />;
  }

  return (
    <div className="app-layout">
      {/* Left sidebar */}
      <aside className="sidebar">
        <div className="sidebar-header">
          <span className="logo">Cowork</span>
        </div>

        <div className="sidebar-section">
          <div className="sidebar-section-title">作業フォルダ</div>
          <div className="sidebar-item active">
            <span className="sidebar-icon">📁</span>
            <span className="sidebar-text" title={workingDir}>
              {workingDir.split(/[/\\]/).pop() || workingDir}
            </span>
          </div>
        </div>

        <div className="sidebar-section">
          <div className="sidebar-section-title">スキル</div>
          <div className="sidebar-item disabled">
            <span className="sidebar-icon">⚡</span>
            <span className="sidebar-text">準備中...</span>
          </div>
        </div>

        <div className="sidebar-section">
          <div className="sidebar-section-title">TODO</div>
          <div className="sidebar-item disabled">
            <span className="sidebar-icon">☑</span>
            <span className="sidebar-text">準備中...</span>
          </div>
        </div>

        <div className="sidebar-footer">
          <button className="sidebar-action" onClick={clearMessages}>
            会話をクリア
          </button>
          <button
            className="sidebar-action"
            onClick={() => changeWorkingDir("")}
          >
            フォルダ変更
          </button>
        </div>
      </aside>

      {/* Main chat area */}
      <main className="chat-main">
        <div className="chat-header">
          <span>チャット</span>
          {isLoading && <span className="loading-badge">応答中...</span>}
        </div>

        <div className="chat-messages">
          {messages.length === 0 && !streamingText && (
            <div className="chat-welcome">
              <div className="welcome-title">Coworkへようこそ</div>
              <p className="welcome-text">
                何でも話しかけてください。ファイルの整理、文書の作成、
                データの処理など、お手伝いします。
              </p>
              <div className="welcome-examples">
                <div
                  className="example-chip"
                  onClick={() =>
                    sendMessage("このフォルダにあるファイルを一覧表示して")
                  }
                >
                  このフォルダの中身を見せて
                </div>
                <div
                  className="example-chip"
                  onClick={() =>
                    sendMessage("このフォルダの構成を説明して")
                  }
                >
                  フォルダ構成を説明して
                </div>
              </div>
            </div>
          )}

          {messages.map((msg) => (
            <MessageBubble key={msg.id} message={msg} />
          ))}

          <StreamingIndicator text={streamingText} />

          {error && (
            <div className="error-banner">
              エラーが発生しました: {error}
            </div>
          )}

          <div ref={chatEndRef} />
        </div>

        <ChatInput onSend={sendMessage} disabled={isLoading} />
      </main>

      {/* Right activity panel */}
      <aside className="activity-sidebar">
        <ActivityPanel activities={activities} />
      </aside>
    </div>
  );
}

export default App;
