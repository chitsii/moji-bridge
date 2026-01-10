# Claude Input

日本語IMEに対応した Claude Code 用入力ヘルパーツール。

## 概要

Claude Code のターミナルでは日本語IMEが正しく動作しないため、外部GUIウィンドウを使用して入力を行うツールです。

## 機能

### 常駐モード
- Claude Code 起動と同時に常駐ウィンドウを起動（バックグラウンド）
- カスタムラベルをウィンドウ上部に表示可能
- **Ctrl+Enter** で入力内容をターミナルに直接送信
- ウィンドウは閉じずに次の入力が可能
- 同じターミナルで複数のClaudeセッションを起動しても、ウィンドウは1つだけ
- **?** ボタンでショートカットキーのヘルプを表示

### グローバルホットキー (Ctrl+I)
- **Claude Codeのターミナルがアクティブな時のみ有効**
- Ctrl+I を押すと claude-input ウィンドウにフォーカスが移動
- 他のアプリケーションでは通常の Ctrl+I 動作を維持

## インストール（Windows）

### 前提条件

- **Rust ツールチェーン**: [rustup](https://rustup.rs/) でインストール
- **Claude Code**: インストール済みであること

### 手順

#### 1. ソースコードの取得

```bash
git clone https://github.com/your-repo/claude-input.git
cd claude-input
```

#### 2. ビルド

```bash
cargo build --release
```

ビルド成果物: `target/release/claude-input.exe`

#### 3. 実行ファイルの配置

任意の場所に配置してください（例: `~/.local/bin/`）

```bash
mkdir -p ~/.local/bin
cp target/release/claude-input.exe ~/.local/bin/
```

#### 4. Claude Code フックの設定

ユーザーホームの `.claude/settings.json` を編集します:

**設定ファイルの場所**: `C:\Users\<ユーザー名>\.claude\settings.json`

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "C:\\Users\\<ユーザー名>\\.local\\bin\\claude-input.exe --detach"
          }
        ]
      }
    ]
  }
}
```

> **注意**: パスの `\` はJSONでは `\\` とエスケープが必要です。

#### 5. 動作確認

1. Claude Code を起動: `claude`
2. claude-input ウィンドウが自動的に起動することを確認
3. ターミナルで **Ctrl+I** を押してウィンドウにフォーカスが移ることを確認
4. テキストを入力し、**Ctrl+Enter** で送信できることを確認

### トラブルシューティング

| 問題 | 対処法 |
|------|--------|
| ウィンドウが起動しない | settings.json のパスが正しいか確認 |
| Ctrl+I が効かない | Claude Code のターミナルがアクティブか確認 |
| 送信されない | ターミナルにフォーカスが戻っているか確認 |

## 使用方法

### カスタムラベル

ウィンドウにプロジェクト名を表示したい場合:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "C:\\Users\\<ユーザー名>\\.local\\bin\\claude-input.exe --detach --label \"My Project\""
          }
        ]
      }
    ]
  }
}
```

### コマンドライン引数

| 引数 | 説明 |
|------|------|
| `--detach` | バックグラウンドで常駐プロセスを起動（**必須**） |
| `--label <NAME>` | ウィンドウに表示するラベル（オプション） |

### 使用フロー

1. Claude Code を起動
2. SessionStart フックにより常駐ウィンドウが自動起動（バックグラウンド）
3. ターミナルで **Ctrl+I** を押して claude-input ウィンドウにフォーカス
4. テキストを入力
5. **Ctrl+Enter** を押して送信
   - クリップボードにテキストがコピーされる
   - ターミナルにフォーカスが移動
   - テキストがペーストされて送信される
6. 入力欄がクリアされ、次の入力が可能

### キーボードショートカット

| ショートカット | 場所 | 動作 |
|--------------|------|------|
| **Ctrl+I** | ターミナル | claude-input ウィンドウにフォーカス |
| **Ctrl+Enter** | claude-input | テキストを送信 |

## 依存関係

- `iced` - GUIフレームワーク
- `clap` - コマンドライン引数パーサー
- `arboard` - クリップボード操作
- `enigo` - キー入力シミュレーション
- `windows` - Windows API（ウィンドウ操作、ホットキー）
- `serde` / `serde_json` - JSON シリアライズ
- `sysinfo` - プロセス情報取得

## ライセンス

MIT
