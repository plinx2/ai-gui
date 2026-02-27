# AI Chat

Tauri 2 (Rust) + React + TypeScript で構築した、AI エージェント搭載のデスクトップチャットアプリケーションです。

## 概要

Google Gemini API を使った会話 AI に、シェル実行・ブラウザ操作などのツール実行能力を持たせたエージェント型チャットアプリです。ローカルで動作し、会話履歴はすべてマシン上の JSON ファイルに保存されます。

## 画面

```
+---------------------------+--------------------------------------------+
|  サイドバー               |  チャット画面                              |
|  [+ New Chat]             |                                            |
|  [セッション 1]           |  [メッセージ一覧（スクロール可能）]        |
|  [セッション 2]    ←→    |  [ツール実行ブロック（折りたたみ）]        |
|  ...                      |                                            |
|  ⚙ Settings               |  [入力欄 / 選択肢パネル]                  |
+---------------------------+--------------------------------------------+
```

## 主な機能

### AI エージェント
- **Gemini API** を使った会話 AI（gemini-2.5-flash / pro、gemini-1.5-flash / pro）
- **エージェントループ**: ツール呼び出し → 実行 → 結果フィードバック → 再回答、を自動繰り返し
- **セッション管理**: 複数の会話を独立したセッションとして保存・切り替え
- **タイトル自動生成**: 最初のメッセージから会話タイトルを AI が生成
- **ファイル添付**: 画像などのファイルを会話に添付可能（Gemini Vision 対応）

### 搭載ツール

| ツール | 説明 |
|---|---|
| `run_shell_command` | シェルコマンドを実行（Windows: PowerShell） |
| `get_current_time` | 現在時刻を返す |
| `present_choices` | ユーザーへの選択肢提示（AI が途中で選択を求める） |
| `browser_search` | Google 検索（直接 URL ナビゲート） |
| `browser_navigate` | 指定 URL を Chrome で開く |
| `browser_get_url` | 現在の URL を取得 |
| `browser_get_text` | 現在ページのテキストを取得 |
| `browser_get_links` | ページ上のリンク一覧を JSON で取得 |
| `browser_click` | CSS セレクタで要素をクリック |
| `browser_type` | 入力フィールドにテキストを入力 |
| `browser_screenshot` | スクリーンショットを撮影（Gemini Vision に渡す） |
| `browser_close` | ブラウザを閉じる |

### インタラクティブ選択肢
AI が処理の途中でユーザーに選択を求める場合、入力欄の代わりに選択肢パネルが表示されます。自由記述での回答も可能です。

### ブラウザ自動操作
インストール済みの Google Chrome を CDP（Chrome DevTools Protocol）経由で自動操作します。ChromeDriver 不要。

**ブラウザ操作の例:**
```
引っ越しを考えています。「札幌市 不動産」で検索して条件に合う物件を探してください。
```
→ Chrome が起動 → Google 検索 → 結果ページを読み取り → 物件サイトに移動 → 情報を抽出 → ユーザーに提示

## 技術スタック

| レイヤー | 技術 |
|---|---|
| デスクトップフレームワーク | [Tauri 2](https://v2.tauri.app/) (Rust) |
| フロントエンド | React 19 + TypeScript |
| スタイリング | Tailwind CSS 4 |
| ビルドツール | Vite 7 |
| AI モデル | Google Gemini API (REST) |
| ブラウザ自動化 | [chromiumoxide](https://github.com/mattsse/chromiumoxide) (CDP) |
| 非同期ランタイム | Tokio |

## アーキテクチャ

### Rust バックエンド (`src-tauri/src/`)

```
lib.rs                    # AppState + Tauri コマンド
config.rs                 # 設定の読み書き
agent.rs                  # Agent struct + ループ
agent/
├── model.rs              # async Model trait
├── tool.rs               # async Tool trait
├── session.rs            # Session / Message 型定義
├── session_store.rs      # SessionStore trait（差し替え可能）
├── knowledge_store.rs    # KnowledgeStore trait（未実装・差し込み口のみ）
├── models/gemini.rs      # Gemini REST API クライアント
├── tools/
│   ├── time.rs           # TimeTool
│   ├── shell.rs          # ShellTool
│   ├── choice.rs         # ChoiceTool (oneshot チャネル + Tauri イベント)
│   └── browser.rs        # ブラウザツール群
└── stores/
    └── local_session_store.rs  # JSON ファイル保存
```

**設計のポイント:**
- `SessionStore` trait によりセッション保存先を差し替え可能（現在: ローカル JSON）
- `KnowledgeStore` trait によりナレッジ拡張の差し込み口を確保（未実装）
- `send_message` 中は Mutex ロックを最小限に保持（API 呼び出し中はロック解放）
- `ChoiceTool` は `tokio::sync::oneshot` チャネルでエージェントループを一時停止し、ユーザー選択を待機

### TypeScript フロントエンド (`src/`)

```
App.tsx                   # chat / settings ルーティング
api.ts                    # invoke() ラッパー
hooks/
├── useSessions.ts        # セッション一覧管理
└── useChat.ts            # アクティブセッション管理（楽観的更新）
components/
├── layout/               # MainLayout, Sidebar
├── chat/                 # ChatView, MessageList, MessageBubble,
│                         # ToolCallBlock, ChatInput, ChoicePanel
└── settings/             # SettingsView
```

## セットアップ

### 必要環境

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/)
- [Google Chrome](https://www.google.com/chrome/)（ブラウザツール使用時）
- Gemini API キー（[Google AI Studio](https://aistudio.google.com/) で取得）

### 開発起動

```bash
pnpm install
pnpm tauri dev
```

### プロダクションビルド

```bash
pnpm tauri build
```

生成物:
- `src-tauri/target/release/ai-gui.exe` — 単体実行ファイル
- `src-tauri/target/release/bundle/msi/` — MSI インストーラー
- `src-tauri/target/release/bundle/nsis/` — NSIS インストーラー

### 初回設定

1. アプリを起動
2. 左下の **Settings** をクリック
3. **Gemini API Key** を入力して **Save**

## データ保存先

設定画面の **Storage** セクションに表示されます。

| データ | 場所 |
|---|---|
| 設定（API キーなど） | `%APPDATA%\net.plinx2.ai-gui\config.json` |
| セッション履歴 | `%APPDATA%\net.plinx2.ai-gui\sessions\{id}.json` |

## ライセンス

MIT
