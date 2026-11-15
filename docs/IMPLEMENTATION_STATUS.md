# STATEFLOW 実装状況

更新日: 2026-11-16（Asia/Tokyo）

## 全体状況

Rust + Tauri 2 + React/TypeScript のアプリ実装、Windows用MSI/NSISビルド、Golden workbookを用いた手動スモークを行った。仕様書11章の全受入項目はまだ確認できていないため、**最終受入は未完了**として扱う。

## フェーズ別チェック

- [x] **Phase 0 — 参照監査と計画**: 実装仕様、v6 HTML、旧Excel、Readable Codeルールを確認し、初期チェックリストを作成。
- [~] **Phase 1 — Tauri起動とUI骨格**: Tauriが起動し、v6の6画面を実データで表示・遷移できる。実機で確認した表示は1602×1032のみ。1366×768 / 1440×900 / 1920×1080での比較は未実施。
- [x] **Phase 2 — Excel / DSL / 正規化モデル**: 6シートのGolden workbook、異なるIDを持つ2ブックの統合、必須列を欠くExcelの診断位置をRustの回帰テストで確認。Rustのトークナイザ/AST/型検証を実装。
- [x] **Phase 3 — 手動グラフ接続と順序**: 取込直後の接続0本、CURRENT / PREVIOUS / FLOW_ONLY、左→右順序、保存・再読込を実装。実機でCURRENTを2本手動接続して保存・再読込した。
- [x] **Phase 4 — Rust 1ステップ解析**: ゴールデンA〜Gを含むRustコアのテスト20件が通過。逐次CURRENT、PREV/BEFORE/STATE、優先順位、UNKNOWN、競合の根拠をトレースする。
- [x] **Phase 5 — 実マトリクスと大規模データ**: 窓取得上限と仮想表示を実装。8機能×4状態×20イベントの試験では理論上10,485,760セルを作らず16セルの窓だけを取得した。
- [x] **Phase 6 — レビュー/判定/CSV**: 候補抽出、判定とメモ、判定の保存/再読込、候補CSVとマトリクスCSVを実画面で確認した。Golden A/Bシナリオでレビュー候補は21件。
- [~] **Phase 7 — QAと納品**: Windows Tauriリリースビルド、MSI/NSIS生成、実行ファイル起動、6画面の基本スモークと証跡保存を実施。高DPI、キーボード/フォーカス、3サイズでのUI比較、インストーラーの実インストールは未確認。

## 主要契約の実装

- マトリクスはイベントを縦、状態組み合わせの機能別現在状態を横に表示し、各セルには次状態を表示する。
- ノードはExcelから生成するが接続は自動生成しない。CURRENT / PREVIOUS / FLOW_ONLYは手動で接続する。
- 評価順はノードx座標の左から右へ、全機能を各1回。後続機能は更新後のSTATEを読む。
- STATE、BEFORE.STATE、PREV.STATE、PREV.OUTPUTを分離し、未設定値を推測補完しない。
- ガードはRustのパーサと型検証で扱う。TypeScript側に本番解析計算を置かない。
- 直積は遅延生成し、セル窓APIと50,000セル上限を通す。無制限の全件転送を行わない。
- 状態維持や候補を設計不具合確定・デッドロック確定と表示しない。

## QA証跡

- [matrix-golden-a.png](qa/matrix-golden-a.png): 前回出力OFFで AUTO / ON / ON。
- [matrix-golden-b.png](qa/matrix-golden-b.png): 前回出力ONで AUTO / ON / OFF。
- [graph-wired-release.png](qa/graph-wired-release.png): リリースアプリでAC → LINK → HEATERを手動接続。
- [TEST_REPORT.md](TEST_REPORT.md): 実行コマンド、環境、結果、未確認事項。

## 残作業

1. 仕様書11章のUI忠実度項目を3つの画面サイズと高DPIで確認し、キーボード/フォーカス、エラー状態を追記する。
2. 複数Excel統合とスキーマ不正Excelの正負fixtureを追加し、元行・列を含む診断を回帰テストする。
3. インストーラーをWindowsへ実際にインストールして起動確認し、仕様書11章の未達チェックを再監査する。

## Phase 0の初期監査記録

開始時はTauri/Rust/Reactの実装骨格およびGit repositoryがなく、旧Excelは5シートのみだった。正式な入力契約は仕様書の6シートを採用する。WindowsエディションとMicrosoft C++ Build Toolsの事前状態は当初未確認だったが、Tauriのリリースビルドは後続の実行環境で成功した。
