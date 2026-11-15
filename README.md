# STATEFLOW — 状態遷移連動アナライザー

複数のExcel状態遷移モデルを統合し、イベント変化と全現在状態の組み合わせに対して各機能を左から右へ1回ずつ評価するWindowsデスクトップアプリです。解析、Excel取込、保存形式、マトリクス窓取得の正はRustの `stateflow-core` にあります。

## 開発起動

WindowsのPowerShellで、Node.js 20.19+ または 22.12+、Rust stable MSVC、Microsoft C++ Build Tools、WebView2 Runtimeを準備します。

```powershell
npm ci
npm run tauri -- dev
```

## アプリの使い方

1. [標準入力テンプレート v1](templates/stateflow_input_v1.xlsx)をコピーして、実仕様の機能・状態・入力・イベント・ルールに置き換えます。これはAC / LINK / HEATERのサンプルデータ入りで、ヘッダーに列の説明があります。
2. 右上の「Excel取込」から正式フォーマットのExcelを1件以上選択します。ブックは `Machines`, `States`, `Inputs`, `Outputs`, `Events`, `Transitions` の6シートを使います。
3. 「依存関係・実行順序」で機能ノードを確認し、CURRENT / PREVIOUS / FLOW_ONLY接続を必要に応じて手動で作成します。取込だけで接続線は自動生成されません。ノードの左から右の順が評価順です。
4. 「統合マトリクス」で固定入力、前回出力、前回状態を指定し、イベント行と状態組み合わせのセルを選んで評価トレースを確認します。縦軸はイベント、横軸は各機能の現在状態、セルは次状態です。
5. 「連動レビュー候補」でRustコアが抽出した候補を確認し、設計者が判定とメモを保存します。候補は設計不具合の確定判定ではありません。
6. 右上の「保存」でローカルのSTATEFLOW JSONとして保存し、あとで「開く」から再読込します。各画面のCSV操作でマトリクス結果またはレビュー候補を出力できます。

この評価は全機能の1ステップ確認です。実機の周期、割込、並行動作を再現するものではありません。条件不成立や状態維持だけでデッドロックや設計不具合と断定しません。

## 検証

```powershell
npm run typecheck
npm test
npm run build
cargo fmt --all -- --check
cargo test -p stateflow-core
cargo clippy -p stateflow-core --all-targets -- -D warnings
npm run tauri -- build
```

`package.json`にはnpm lint scriptはありません。詳細な検証環境、結果、未確認項目は [docs/TEST_REPORT.md](docs/TEST_REPORT.md)、画面証跡は [docs/qa](docs/qa/) を参照してください。

## Windows配布物

- NSIS: `target/release-v0.0.1/release/bundle/nsis/STATEFLOW_0.0.1_x64-setup.exe`
- MSI: `target/release-v0.0.1/release/bundle/msi/STATEFLOW_0.0.1_x64_en-US.msi`
- EXE: `target/release-v0.0.1/release/stateflow.exe`

アプリの仕様、フェーズ状況、既知の受入未完了項目は [STATEFLOW_実装指示書.md](STATEFLOW_実装指示書.md) と [docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md) を参照してください。

## AI / 開発者向けドキュメント

- [AI向け引き継ぎ書](docs/AI_HANDOFF.md): 最短のコードマップ、実装済み契約、既知の差分、次の作業。
- [技術設計書](docs/ARCHITECTURE.md): Rust / Tauri / Reactの責務、評価・グラフ・Excel・レビューの詳細。
- [正式Excel入力テンプレート v1](templates/stateflow_input_v1.xlsx): 6シートと架空の動作例。
- [検証記録](docs/TEST_REPORT.md) / [実装状況](docs/IMPLEMENTATION_STATUS.md) / [判断記録](docs/DECISIONS.md)
