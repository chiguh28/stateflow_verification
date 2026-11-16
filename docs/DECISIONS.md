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

## ドキュメント・入力テンプレート

### D-008: 正式入力テンプレートは説明付きのGoldenサンプルにする

- **背景:** 仕様書4章は `templates/stateflow_input_v1.xlsx` を正式テンプレートとして要求している。旧版は5シートで、入力イベントと前回出力の定義が不足していた。
- **採用判断:** 6シート・仕様書準拠の列を揃えたv1 workbookを作成し、架空のAC / LINK / HEATERサンプルを入れる。列ヘッダーのコメント、固定ヘッダー、フィルター、可読な幅とスタイルを付ける。テンプレート内には取込対象外の説明シートを増やさない。
- **理由:** 完全空白のブックよりも、型・ガード式・イベント行の表現を具体例で示せる。既存のRust Golden A/Bに対応するため、取込後の状態遷移を再現しやすい。
- **検証:** `importer_workbooks.rs`で正式ファイルを実際に読む。3機能、6状態、3入力、1出力、4イベント、6ルール、取込時の接続0本、2種類の未接続CURRENT辺について4件のルール単位診断を確認する。
- **残るリスク:** サンプルデータは説明用であり、実プロジェクトの仕様正しさを保証しない。利用者はサンプルを実仕様へ置換し、CURRENT辺2本をアプリ上で手動接続する必要がある。

### D-009: AI向け設計資料はソースコードの再調査を減らす索引として追加する

- **採用判断:** `docs/ARCHITECTURE.md`に責務・データフロー・解析契約とコードマップを、`docs/AI_HANDOFF.md`に実装済み範囲・既知差分・再開手順を記録する。仕様書とv6 HTMLを置き換えず、未達と差分も明示する。
- **理由:** 次のAIが、機能意図をソース全体から再構築したり、プロトタイプのダミー処理を本番仕様として誤読したりする時間を減らす。
- **残るリスク:** コード変更時に資料が古くなる。解析やExcel契約を変更したPRでは本書と引き継ぎ書も更新する。

### D-010: Windows初回起動はBATからPowerShellセットアップを呼び出す

- **背景:** clone直後のWindows環境にはTauri開発に必要なMSVC、Rust MSVC toolchain、Node.jsが揃っているとは限らない。手動導入だけでは初回起動までの手順が分散する。
- **採用判断:** リポジトリ直下の`setup-and-run.bat`から`scripts/setup-and-run.ps1`を実行し、wingetでVisual Studio 2022 C++ Build Tools（推奨コンポーネントを含む）、Rustup、Node.js LTSを用意する。WebView2 Runtimeが見つからない場合だけwingetから追加する。npm lockfileのハッシュに応じて`npm ci`を行い、Visual Studioの`vcvars64.bat`環境でTauriを開発起動する。
- **理由:** BATだけに依存管理、Visual Studio環境の取込、再実行時の依存判定を詰め込まず、Windows PowerShell 5.1で読める個別処理として保守できる。Rustの既定toolchainを変えず、起動プロセスだけMSVC toolchainを選ぶ。
- **制約:** Windows 10/11、winget、ネット接続が必要。Visual Studio Build Toolsの導入時はUAC承認と数GBの空き容量が必要。組織ポリシーやオフライン環境では自動導入できない。
- **残るリスク:** clean Windows環境でのwinget初回インストールは、この開発機では再現できないため別環境で確認が必要。WebView2 Runtimeは多くのWindows 10/11に同梱されるが、ランタイム未導入環境では追加ダウンロードが発生する。
