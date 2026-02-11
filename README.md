# Cowork

Claude Code の機能を、ターミナルを使わずにデスクトップアプリとして利用できる GUI ラッパーです。
Claude CoworkがWindowsで出てこなくて腹たったので作りました。多分すぐ出ると思うのでそれまでの間だけです。

Windows / macOS / Linux に対応しています。

## WIP (Work In Progress)

**このプロジェクトは開発初期段階です。** 多くの機能が未実装または不安定な状態にあります。

動作する機能:

- Claude Code とのチャット（ストリーミング表示）
- ツール実行状況の日本語リアルタイム表示
- セッション継続（会話の永続化）
- ファイルブラウザ
- スキル管理（作成・実行・削除）
- Slack / Google Drive の OAuth 認証

まだ動かない・不安定な機能:

- 承認ダイアログ（Claude Code との接続が不完全）
- Google Drive ファイル操作の UI
- Slack リストとの TODO 同期
- その他、[docs/features.md](docs/features.md) を参照

## 概要

Claude Code は強力な CLI ツールですが、ターミナル操作に馴染みのないユーザーにとっては敷居が高いものです。Cowork は Claude Code の CLI を内部的にラップし、以下を提供します:

- **チャット UI** -- Claude との対話をメッセージアプリのように表示
- **ツール実行の可視化** -- Bash, Read, Write 等のツール呼び出しを日本語に翻訳してリアルタイム表示
- **承認の日本語化** -- ファイル削除やコマンド実行の権限確認を分かりやすく表示
- **スキル** -- よく使う手順を Claude Code スキルとして保存・ワンクリック実行

## 技術スタック

| レイヤー       | 技術                                 |
| -------------- | ------------------------------------ |
| フレームワーク | Tauri v2                             |
| バックエンド   | Rust                                 |
| フロントエンド | React + TypeScript                   |
| Claude 連携    | `claude --output-format stream-json` |

## セットアップ

[INSTALL.md](INSTALL.md) を参照してください。

## ライセンス

[BSD 3-Clause License](LICENSE)
