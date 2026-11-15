# STATEFLOW 実装上の判断

## Phase 0 — 監査

### D-001: 既存デモの解析・保存処理を本番へ移植しない

- **現象:** v6 HTMLには固定のAC/LINK/HEATERデモ計算とブラウザlocalStorage保存がある。
- **衝突する仕様:** v6は視覚・操作の承認参照。一方で実装仕様はRustコアを解析の唯一の正とし、ローカルJSONプロジェクトを保存形式とする。
- **採用判断:** v6から構造・文言・スタイル・操作導線を再実装する。デモ解析はPhase 1内の隔離fixtureだけに置き、Excelを開いた本番経路では使用しない。
- **残るリスク:** 画面の見た目だけでなく、操作の意味を実画面で比較しながら調整する必要がある。

### D-002: 正式Excel v1を新規に作る

- **現象:** 旧Excelには `README`, `Machines`, `States`, `Inputs`, `Transitions` の5シートしかなく、`Outputs` と `Events` がない。
- **衝突する仕様:** 旧テンプレートは参照用。正式フォーマットは6シートで、入力イベントと前回出力宣言を明示する。
- **採用判断:** 旧ブックは不足シート名と列を示す診断のfixtureとして扱う。正式テンプレートは仕様書4章どおり新規生成する。
- **残るリスク:** 旧テンプレートの個別行を正例として使う場合、イベントを暗黙生成せず、人が正式版へ補う必要がある。

### D-003: UIと解析責務を分離する

- **現象:** UI要件には大規模マトリクス、手動接続、レビュー詳細が含まれる。
- **採用判断:** Reactは表示・入力に限定し、解析、Excel取込、永続化、窓取得は `stateflow-core` / Tauriへ置く。マトリクスはevent/combo窓APIと二方向仮想化を用いる。
- **残るリスク:** UI DTOとRust型の同期をビルドで確認できるようにし、レビュー強調を解析の事実と分離する。

### D-004: Phase 1の依存は公式の現行Tauri 2手順に沿って固定する

- **現象:** リポジトリにpackage/tauri設定がなく、CLIもPATHにない。
- **採用判断:** Windows用の公式 `create-tauri-app` React + TypeScript/Vite構成を起点にし、npm lockfileを保存する。Rust coreはTauri依存のないworkspace crateにする。Tauri filesystem accessは狭い capability とネイティブ選択ダイアログを使う。
- **確認資料:** [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/), [Tauri 2 create project](https://v2.tauri.app/start/create-project/), [Tauri filesystem plugin permissions](https://v2.tauri.app/plugin/file-system/), [React versions](https://react.dev/versions), [Vite guide](https://vite.dev/guide/), [React Flow quick start](https://reactflow.dev/learn), [TanStack Virtual installation](https://tanstack.com/virtual/latest/docs/installation).
- **残るリスク:** 公式ガイドは最新版を参照しているため、実際に解決されたnpm/crateバージョンはlockfileを正とし、起動・ビルドで互換性を確認する。

## Phase 3 — 実機接続QA

### D-005: 初期ノード間隔を340pxにする

- **現象:** 3ノードを240px間隔にすると、250px幅のカードが横方向に重なり、ポートをドラッグして接続する操作が難しかった。
- **採用判断:** インポート時の初期x座標を0 / 340 / 680にする。接続は従来どおり0本で、ユーザーがCURRENT / PREVIOUS / FLOW_ONLYを手動接続する。
- **確認:** WindowsリリースアプリでAC → LINK、LINK → HEATERをドラッグ接続し、保存して開き直した後も2本が復元することを確認。
- **残るリスク:** ノード数が多い場合の画面幅やパン/ズーム操作は、別のUIサイズで追加確認が必要。

## Phase 5 — 遅延取得の負荷試験

### D-006: 大規模直積は窓APIの取得数と時間を記録する

- **現象:** 理論上の全セル数をDOMやIPCに積まず、APIの窓取得だけで上限を守る必要がある。
- **採用判断:** Rustテストで8機能×4状態×20イベントの直積を作り、理論上の10,485,760セルに対して1イベント×2組み合わせの窓だけを要求する。
- **確認:** 全件要求は50,000セル上限で拒否され、16セルの窓取得は今回の再実行で6.3521ms。前回測定は590.9µsだった。プロセスRSS/ピークRAMは計測していない。
- **残るリスク:** 実行時間は自動テスト環境で変動し、実運用速度やRAM上限を保証しない。

## Phase 7 — 受入範囲

### D-007: UIの最終受入は複数表示サイズとアクセシビリティ確認後に行う

- **現象:** 1602×1032のリリース画面と6画面遷移は実機確認したが、仕様書記載の他の表示サイズ、高DPI、キーボード/フォーカス確認をしていない。
- **採用判断:** これらの未確認項目を完了扱いにせず、Phase 7を部分完了として報告する。
- **確認:** UI証跡とビルド記録はdocs/qaおよびdocs/TEST_REPORT.mdに保存した。
- **残るリスク:** 仕様書11章の1項目でも未達なら完成と表現しない。
