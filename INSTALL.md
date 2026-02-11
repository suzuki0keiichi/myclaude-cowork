# インストール手順

## 前提条件

以下がインストールされている必要があります:

- [Node.js](https://nodejs.org/) v18 以上
- [pnpm](https://pnpm.io/) v8 以上
- [Rust](https://rustup.rs/) (最新の stable)
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) CLI (`claude` コマンドが使える状態)

### プラットフォーム固有の依存

**macOS:**
- Xcode Command Line Tools (`xcode-select --install`)

**Linux (Ubuntu/Debian):**
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

**Windows:**
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- WebView2 (Windows 10/11 には通常プリインストール)

## ビルド

```bash
# リポジトリのクローン
git clone https://github.com/suzuki0keiichi/myclaude-cowork.git
cd myclaude-cowork

# 依存パッケージのインストール
pnpm install

# 開発モードで起動
pnpm tauri dev

# リリースビルド
pnpm tauri build
```

リリースビルドの成果物は `src-tauri/target/release/bundle/` に出力されます。

## Claude Code の設定

Cowork を使うには、あらかじめ Claude Code CLI のセットアップが必要です:

1. Claude Code をインストール: `npm install -g @anthropic-ai/claude-code`
2. 認証を完了: `claude` を一度実行してログイン
3. Cowork を起動し、作業フォルダを選択
