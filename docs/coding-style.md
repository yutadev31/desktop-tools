# Rust コーディングスタイル

この規約は workspace の全5 crate（desktop-icons、rubar、rumenu、shell-surface、shot）と、今後追加する crate に適用する。CLI、設定形式、実行時の振る舞いを保ちながら、読みやすさ、保守性、実行時性能を優先する。

## 書式と配置

stable rustfmt の標準設定と Rust の命名慣例に従う。独自の rustfmt 設定は追加しない。import は標準ライブラリ、外部 crate、自 crate の順に空行で区分し、同じパスの import はまとめる。glob import はテスト内の `use super::*` だけに認める。

宣言は、モジュールドキュメント、`mod`、`use`、定数・型、実装・関数、テストの順を基本とする。型とその実装は近くに置く。属性とドキュメントコメントは対象の宣言に密着させ、関数・型などの宣言間と import の区分間は1行空ける。

## 空行

`if`、`match`、`for`、`while`、`loop` を含む文の前後に空行を1行入れる。`if let`、`while let`、制御構文を含む代入・戻り値の式も対象とする。短いガード節にも一律に適用し、隣接する `return` などとの間も空ける。

ブロックの先頭・末尾には空行を入れない。制御構文が最初・最後の処理なら、開き括弧・閉じ括弧との間は空けない。`else`・`else if` の接続部分と `match` の arm 間は空けない。arm 内には通常のルールを適用する。

複数行の呼び出し・メソッドチェーンという理由だけでは空行を追加しない。通常の文同士も折り返し行数を理由に区切らない。連続する空行は禁止する。rustfmt が決定する整形を優先する。

連続するガード節:

```rust
fn first(items: &[u32], enabled: bool) -> Option<u32> {
    if !enabled {
        return None;
    }

    if items.is_empty() {
        return None;
    }

    Some(items[0])
}
```

代入の `match` と関数末尾の式:

```rust
fn label(value: Option<&str>) -> String {
    let prefix = "item";

    let value = match value {
        Some(value) => value,
        None => "unknown",
    };

    if value.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix}: {value}")
    }
}
```

入れ子と arm 内の処理:

```rust
fn collect(items: &[Option<u32>]) -> Vec<u32> {
    let mut values = Vec::new();

    for item in items {
        match item {
            Some(value) => {
                let value = *value;

                if value == 0 {
                    continue;
                }

                values.push(value);
            }
            None => break,
        }
    }

    values
}
```

## 設計とエラー処理

- 早期 return でネストを抑える。単純な変換・収集には iterator、副作用や複雑な分岐にはループを使う。
- 公開範囲を必要最小限にし、不要な clone、確保、抽象化を避ける。
- エラーには原因と操作対象を保持する。外部利用する API は型付きエラーを使う。アプリ内部や最上位では `Box<dyn Error>` を認める。
- 回復可能な失敗に `unwrap`・`expect` を使わない。テストでは認める。
- `unsafe` は最小範囲にし、成立条件を英語の `SAFETY` コメントで説明する。
- コメント、API ドキュメント、エラーメッセージは英語で書く。公開 API の契約と、該当する `# Errors`・`# Panics`・`# Safety` を記載する。
- 一時的な回避策ではなく根本原因を修正する。エラーを黙殺せず、原因を追えるようにする。外部コマンドよりライブラリや IPC を優先する。
- テストは実装の写しではなく振る舞いを検証する。プラットフォーム固有の変更では検証した対象と未検証の対象を明示する。

## 自動チェックとレビュー

workspace lint の Clippy `all`・`pedantic`・`undocumented_unsafe_blocks` を全 crate で継承する。CI は `-D warnings` で警告をエラーとする。`restriction`・`nursery` は一括導入しない。

lint 回避目的の `allow`・`expect` は使わない。外部 API・FFI など不可避の制約に限り、最小範囲の理由付き例外を認める。

次をリポジトリルートで実行する。

```sh
cargo fmt --all --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
```

空行、import の区分、宣言順など rustfmt と Clippy で強制できない規則はレビューで確認する。独自チェッカーは追加しない。rustfmt 実行後の実コードと上の代表例を照合し、文書・lint 設定・実装の整合性を確認する。

## 今回の公開 API 変更

- `shell_surface::Shell::render` と `shell_surface::Backend::run` は `shell_surface::Error` を返す。描画エラーは `Error::Render { surface, source }`、バックエンドの失敗は `Error::Backend { backend, source }` として扱う。アプリ独自のエラーは `source` に格納し、元の型を保持する。
- `shot::Error::ClipboardFailed` は原因の `arboard::Error` を保持するタプル variant になった。パターンは `ClipboardFailed(source)` または `ClipboardFailed(_)` とする。PNG のデコードエラーは `Error::Image` で返す。

- `shot::Error::{X11Connect, X11Image}` の payload も文字列から原因の boxed error に変更し、`source()` で元の接続・プロトコル・画像エラーを確認できる。

CLI のオプション、設定形式、daemon の通信形式は変更しない。

## 適用時の検証範囲

Linux 上で上記4コマンドを実行し、88テスト成功、手動の SNI サービステスト1件は既存の指定どおり無視した。Unix socket を使用するテストは sandbox 外で実行した。

追加で `shell-surface` のバックエンドなし・Wayland 単独・X11 単独、`shot` の Wayland 単独・X11 単独について Clippy を警告なしで確認した。`shot --no-default-features` で全バックエンドを無効化すると、既存の未使用変数・未使用コードの警告が残る。この構成は今回の CI の全 feature チェック対象には含まれない。

Windows・FreeBSD・NetBSD 固有コードは静的に確認したが、対象 toolchain と実行環境がないため、そのターゲットでのビルド・実行は未検証。実際のデスクトップセッションでの描画・キャプチャも未検証。
