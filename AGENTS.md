# AGENTS.md — STATEFLOW Codex 開発ルール

このリポジトリを変更する全てのCodexエージェントは、**最初に `STATEFLOW_実装指示書.md` と `references/STATEFLOW_UI_Prototype_v6.html` を読むこと**。本ファイルは短縮ガードレールであり、詳細な受入条件は仕様書が優先する。

## 絶対ルール

1. Ver.6の画面構成・Tesla風配色・6画面・ComfyUI風ノードを維持。既存UIを独断で再設計しない。
2. マトリクスは縦軸イベント、横軸は状態組み合わせグループの機能別現在状態列、**セルには次状態だけ**。
3. ノードはExcelから自動追加するが、エッジの自動生成は禁止。手動接続のCURRENT/PREVIOUS/FLOW_ONLYは正式な解析契約。
4. 実行順序は左→右、全機能各1回、後続は最新 `STATE`。機能内ルールは小さい `priority` が優先。
5. `STATE`、`BEFORE.STATE`、`PREV.STATE`、`PREV.OUTPUT`は異なる意味。前回値の黙示補完は禁止。
6. 全組み合わせは遅延展開。大容量セルをDOMやIPCへ全件流さない。
7. 解析ロジックは`stateflow-core`のRustで実装。TSのダミー計算を本番ロジックにしない。
8. ガード式はトークナイザ+AST+型検証で評価。`eval`・勝手な条件補完は厳禁。
9. 状態維持・レビュー候補を「設計不具合確定」「デッドロック確定」と表現しない。
10. 全てのフェーズでテストし、ビルド実行を確認。未実装・未確認・失敗を隠さない。

## 作業方法

- 最初に `docs/IMPLEMENTATION_STATUS.md` にフェーズ別チェックリストを作る。
- 変更は一度に広げず、フェーズ単位で進める。各フェーズのゲートを仕様書に照合する。
- Rustコアテストを最重視。必要なら最初に失敗するテストを書き、修正して緑にする。
- UI変更では実際に描画して参照HTMLとの差分確認。スクリーンショット・原因・結果を残す。
- `docs/DECISIONS.md` に前提、選択肢、採用判断、残るリスクを記録。
- `docs/TEST_REPORT.md` に実行したコマンド、OS、結果、未実行理由を記録。
- コードフォーマット、lint/typecheck、Rust `cargo test`、`cargo clippy`、frontend test/buildを実行する。
- 終了時は「完了項目 / 未完了項目 / テスト / 起動・再現方法」を報告。

## 重要なテスト

ゴールデン例：AC(MANUAL) → LINK(OFF) → HEATER(OFF)。入力`AUTO_SW OFF→ON`, `LINK_ENABLE=ON`, `SAFETY_LOCK=OFF`, `PREV.OUTPUT.HEATER_CMD=OFF`なら**AUTO / ON / ON**。前回出力だけONにすれば**AUTO / ON / OFF**。変わらない理由をPREV条件で説明できる必要がある。

## 優先順位

最優先：モデル/解析の正確性、ユーザー承認UIへの忠実さ、レビューの根拠。次点：大規模データの操作性、保存とエクスポート。装飾や未要求機能追加は後回し。
