import { useRef, useEffect, useState } from "react";
import { useClaude } from "./hooks/useClaude";
import { MessageBubble } from "./components/MessageBubble";
import { ChatInput } from "./components/ChatInput";
import { ActivityPanel } from "./components/ActivityPanel";
import { StreamingIndicator } from "./components/StreamingIndicator";
import { SetupScreen } from "./components/SetupScreen";
import { FileBrowser } from "./components/FileBrowser";
import { TodoPanel } from "./components/TodoPanel";
import { SkillManager } from "./components/SkillManager";
import { SettingsPanel } from "./components/SettingsPanel";
import { ApprovalDialog } from "./components/ApprovalDialog";
import "./App.css";

type SidebarTab = "files" | "skills" | "todos" | "settings";

function App() {
  const {
    messages,
    activities,
    isLoading,
    streamingText,
    workingDir,
    lastWorkingDir,
    error,
    pendingApproval,
    inferenceMode,
    sendMessage,
    cancelMessage,
    changeWorkingDir,
    clearMessages,
    respondToApproval,
    setInferenceMode,
  } = useClaude();

  const chatEndRef = useRef<HTMLDivElement>(null);
  const [sidebarTab, setSidebarTab] = useState<SidebarTab>("files");
  const [selectedFiles, setSelectedFiles] = useState<string[]>([]);

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingText]);

  const handleFileToggle = (path: string) => {
    setSelectedFiles((prev) =>
      prev.includes(path)
        ? prev.filter((p) => p !== path)
        : [...prev, path]
    );
  };

  if (!workingDir) {
    return <SetupScreen onSetup={changeWorkingDir} defaultPath={lastWorkingDir} />;
  }

  return (
    <div className="app-layout">
      {pendingApproval && (
        <ApprovalDialog
          description={pendingApproval.description}
          details={pendingApproval.details}
          onApprove={() => respondToApproval(true)}
          onReject={() => respondToApproval(false)}
        />
      )}

      <aside className="sidebar">
        <div className="sidebar-header">
          <span className="logo">Cowork</span>
        </div>

        <div className="sidebar-tabs">
          <button
            className={`sidebar-tab ${sidebarTab === "files" ? "active" : ""}`}
            onClick={() => setSidebarTab("files")}
          >
            📁 ファイル
          </button>
          <button
            className={`sidebar-tab ${sidebarTab === "skills" ? "active" : ""}`}
            onClick={() => setSidebarTab("skills")}
          >
            ⚡ スキル
          </button>
          <button
            className={`sidebar-tab ${sidebarTab === "todos" ? "active" : ""}`}
            onClick={() => setSidebarTab("todos")}
          >
            ☑ TODO
          </button>
          <button
            className={`sidebar-tab ${sidebarTab === "settings" ? "active" : ""}`}
            onClick={() => setSidebarTab("settings")}
          >
            設定
          </button>
        </div>

        <div className="sidebar-content">
          {sidebarTab === "files" && (
            <FileBrowser
              workingDir={workingDir}
              onFileSelect={(path) =>
                sendMessage(`このファイルの内容を確認して: ${path}`)
              }
              selectedFiles={selectedFiles}
              onFileToggle={handleFileToggle}
            />
          )}
          {sidebarTab === "skills" && (
            <SkillManager
              workingDir={workingDir}
              selectedFiles={selectedFiles}
              onExecute={sendMessage}
            />
          )}
          {sidebarTab === "todos" && <TodoPanel />}
          {sidebarTab === "settings" && <SettingsPanel />}
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

      <main className="chat-main">
        <div className="chat-header">
          <span>チャット</span>
          <select
            className="inference-mode-select"
            value={inferenceMode}
            onChange={(e) => setInferenceMode(e.target.value as "claude" | "local_llm")}
          >
            <option value="claude">Claude Code</option>
            <option value="local_llm">ローカルLLM</option>
          </select>
          <span className="chat-header-path" title={workingDir}>
            {workingDir}
          </span>
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
                <div
                  className="example-chip"
                  onClick={() =>
                    sendMessage("最近変更されたファイルを教えて")
                  }
                >
                  最近の変更を教えて
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

        <ChatInput onSend={sendMessage} disabled={isLoading} isLoading={isLoading} onCancel={cancelMessage} />
      </main>

      <aside className="activity-sidebar">
        <ActivityPanel activities={activities} />
      </aside>
    </div>
  );
}

export default App;
