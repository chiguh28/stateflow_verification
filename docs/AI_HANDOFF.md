# STATEFLOW AI向け引き継ぎ書

更新日: 2026-11-16（Asia/Tokyo） / 対象: STATEFLOW v0.0.1 / Windows Tauriアプリ / Repository: [chiguh28/stateflow_verification](https://github.com/chiguh28/stateflow_verification)

## 30秒で把握する

STATEFLOWは、複数の状態遷移Excelを統合し、入力イベント1件の変化 × 全機能状態の直積をそれぞれ独立した試験として評価するアプリ。RustコアがExcel取込、DSL、型/接続検証、左→右の1回ずつの状態評価、遅延マトリクス、レビュー候補、保存を担当する。Reactは6画面の表示と編集操作、TauriはローカルUIとRustの橋渡しを担当する。

**まず読むもの**: [技術設計書](ARCHITECTURE.md)でコードの所在と実装契約を確認し、変更対象に応じて[実装指示書](../STATEFLOW_実装指示書.md)の該当節と[v6 UI参照](../references/STATEFLOW_UI_Prototype_v6.html)を照合する。Repositoryの`AGENTS.md`は実装指示書とv6 HTMLの事前確認を要求している。要求を勝手に緩めない。

## 絶対に壊してはいけない契約

1. v6の6画面、Tesla風の白/チャコール/グレー/赤、ComfyUI風グラフを維持する。
2. マトリクスは縦軸がイベント、横軸が状態組み合わせごとの機能別現在状態列。セルには次状態だけを表示する。
3. Excel取込は機能ノードのみ追加し、グラフ辺を自動生成しない。CURRENT / PREVIOUS / FLOW_ONLYは手動設定で、検証対象となる契約。
4. ノードx座標順に全機能を各1回評価。後続機能は最新`STATE`を読む。priorityは機能内で別に適用し、小さい値が優先。
5. `STATE`, `BEFORE.STATE`, `PREV.STATE`, `PREV.OUTPUT`を混同せず、未指定の前回値を補完しない。
6. 状態直積を全件DOM/IPCへ送らず、Rust窓APIとUI二方向仮想化を使う。
7. 本番解析は`stateflow-core` Rust。ガードはtokenizer/AST/型検証を通す。JSの`eval`や条件推測を使わない。
8. 保持やレビュー候補から設計不具合/デッドロックを断定しない。
9. 仕様書11章が未達の間はアプリを「最終受入完了」と呼ばない。

## 現在の実装状態

### 動作を確認済み

- 6シートExcelの取込、複数ブック統合、出典位置、型/参照/DSL検証。
- CURRENT / PREVIOUS / FLOW_ONLYの手動グラフ接続、x順による実行順、JSON保存/再読込。
- Rust 1-step評価、Golden A〜G、priority、3値UNKNOWN、同順位競合、CURRENT逐次更新。
- matrix metadata/window、50,000セル窓上限、二方向virtual UI。
- R01/R02/R04/R05のレビュー候補、判定メモの保存、CSV。
- Windows v0.0.1 Tauri build。実行アプリ/インストーラ成果物は[検証記録](TEST_REPORT.md)。

### 次のAIが知っておくべき未完了・差分

- 仕様書11章の最終受入は**未完了**。1602×1032以外の指定画面サイズ、高DPI、keyboard/focus、インストーラー実インストール、エラー状態網羅は未確認。
- `OrderMissedUpdate` / R03はmodel、UI label、review codeに宣言されているが、`crates/stateflow-core/src/review.rs`で候補を生成していない。現在ONでも候補は出ない。
- Excel importerは未知シートを警告するが、未知の追加列を診断しない。仕様書4.1の列チェック要件と差がある。
- Rust評価中にある機能のnext stateがUNKNOWN/競合となると、その後の機能は一意に評価できず、現コードでは評価エラーになる場合がある。部分traceやUNKNOWN伝播を拡張する際、既存UNKNOWNテストと表示契約を決める。
- 複数book・必須列欠損など一部Excel負例はあるが、不正Excelの網羅テストは未完了。
- npm lint scriptがない。CPU/RAMはscale testで未計測。

残作業と根拠は[IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)、コマンドごとの実績は[TEST_REPORT.md](TEST_REPORT.md)を参照する。上記を埋めずに「完成」と表現しない。

## よくある変更の入口

| やりたいこと | まず見る場所 | 必ず合わせて更新 |
| --- | --- | --- |
| Excel列や取込診断を変える | `crates/stateflow-core/src/importer.rs` | `templates/stateflow_input_v1.xlsx`, importer tests, `ARCHITECTURE.md` |
| guard構文を変える | `dsl.rs`, `validation.rs` | `tests/golden_model.rs`, DSL tests, 実装指示書4.3 |
| 評価/優先度/履歴を変える | `engine.rs`, `model.rs` | Golden A〜G, `TEST_REPORT.md`, `ARCHITECTURE.md` |
| グラフ参照検証を変える | `validation.rs`, `GraphPage.tsx` | graph tests, graph screenshot/QA evidence |
| review抽出やIDを変える | `review.rs`, `ReviewSettingsPage.tsx`, `ReviewPage.tsx` | fingerprint/decision tests, review CSV checks |
| IPC/DTOを変える | `src-tauri/src/commands.rs`, `src/lib/tauri.ts` | frontend typecheck/tests and command callers |
| 保存形式を変える | `model.rs`, `project_file.rs` | schema version/migration policy, save/load tests |
| 見た目/操作を変える | `src/features/`, `src/components/`, styles | `references/STATEFLOW_UI_Prototype_v6.html`との実画面比較 |

## Excelサンプルの使い方

[標準入力テンプレート v1](../templates/stateflow_input_v1.xlsx)は架空のAC → LINK → HEATERモデルを含む、直接取り込めるサンプルである。6シートに必要列・例・header commentsを入れている。新規Excelの雛形としてコピーし、ID・状態・ガード・イベント・出典を実仕様へ置き換える。

旧形式を基にした[参照ブック](../references/standard_input_template_legacy.xlsx)にも`Outputs` / `Events`と必須priorityを追加した。新規入力Excelを作る場合は引き続きv1テンプレートを使う。

この例の `STATE.AC` と `STATE.LINK` の参照を解決するには、取込後にGraph画面で`AC → LINK (CURRENT)`と`LINK → HEATER (CURRENT)`を手動接続する。自動配線しないことは仕様である。未接続診断は2種類の必須辺に対し、各辺を参照する2ルールずつ計4件出る。旧形式は[legacy reference](../references/standard_input_template_legacy.xlsx)。

## 開発・検証コマンド

Windows PowerShell:

```powershell
npm ci
npm run tauri -- dev

npm run typecheck
npm test
npm run build
cargo fmt --all -- --check
cargo test -p stateflow-core
cargo clippy -p stateflow-core --all-targets -- -D warnings
npm run tauri -- build
```

`npm run lint`はscript未登録。`cargo test -p stateflow-core`がRust解析の主要ゲート。実行時間やサンドボックスのアクセス拒否を含む過去の実測は[TEST_REPORT.md](TEST_REPORT.md)に残す。

## Golden A/B — 変更時の必須理解

初期状態が`AC=MANUAL, LINK=OFF, HEATER=OFF`、イベント`AUTO_SW OFF→ON`、固定`LINK_ENABLE=ON`, `SAFETY_LOCK=OFF`の場合:

| 前回出力 `HEATER_CMD` | 期待する次状態 | 根拠 |
| --- | --- | --- |
| OFF | `AUTO / ON / ON` | ACが先にAUTOへ変わり、LINKは更新済み`STATE.AC`を読み、HEATERはONを読む。 |
| ON | `AUTO / ON / OFF` | HEATER条件の`PREV.OUTPUT.HEATER_CMD == OFF`が偽になり、HEATERは保持。 |

各イベント行は独立ステップであり、前行のnext stateを次行へ持ち越さない。レビュー判定は入力モデルの正誤を自動確定しない。

## 変更後の手順

1. 変更要求を実装指示書の節と本書のコードマップに結びつける。
2. Rustモデル/解析変更は再現テストを追加し、Golden A〜Gと関連core testsを実行する。
3. UI変更はv6 HTMLと実描画を比べ、スクリーンショットと未確認事項を記録する。
4. Excel仕様変更はバージョン付きテンプレートとfixture、schema説明を同時更新する。
5. `docs/IMPLEMENTATION_STATUS.md`, `docs/DECISIONS.md`, `docs/TEST_REPORT.md`を実際の結果に合わせる。
6. 検証前にPASSと書かない。未達の受入項目を残したまま「最終完成」と報告しない。
