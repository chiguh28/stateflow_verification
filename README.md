# STATEFLOW — 状態遷移連動アナライザー

複数のExcel状態遷移モデルを統合し、イベント変化と全現在状態の組み合わせに対して各機能を左から右へ1回ずつ評価するWindowsデスクトップアプリです。解析、Excel取込、保存形式、マトリクス窓取得の正はRustの`stateflow-core`にあります。

## Windowsでソースを取得して起動する

対象はWindows 10/11 x64です。初回セットアップにはインターネット接続、winget、Visual Studio C++ Build Toolsの導入に必要な空き容量（数GB）を使います。Build Toolsのインストール時にUAC確認が表示されたら承認してください。

### 1. リポジトリを取得

Gitがまだインストールされていない場合はPowerShellで一度だけ実行します。

```powershell
winget install --id Git.Git --exact --source winget
```

続けてリポジトリをcloneします。

```powershell
git clone https://github.com/chiguh28/stateflow_verification.git
Set-Location stateflow_verification
```

Gitを使わない場合はGitHubのCode → Download ZIPから取得して展開しても起動できます。

### 2. セットアップして起動

リポジトリ直下の`setup-and-run.bat`をダブルクリックします。またはPowerShellから実行します。

```powershell
.\setup-and-run.bat
```

このスクリプトは不足している項目を確認してから次を行い、STATEFLOWの開発版を起動します。

- wingetがない場合は、Microsoft Storeの「アプリ インストーラー」を入れてから再実行するよう案内します。
- Visual Studio 2022 C++ Build ToolsのVC Tools workloadと推奨コンポーネント（Windows SDKを含む）を導入します。インストーラーが管理者承認を求めます。
- Rustupと`stable-x86_64-pc-windows-msvc` toolchainを導入します。既定toolchainの設定は変更しません。
- Node.js LTSを導入します。Vite 8の要件に合わせ、Node.js 20.19以降または22.12以降が必要です。
- WebView2 Runtimeが見つからない場合に限り、wingetから導入します。
- `package-lock.json`に変更がある場合、または依存関係がない場合に`npm ci`を実行します。
- MSVCの開発環境を設定してから`npm run tauri -- dev`を実行します。

初回はVisual Studio Build Tools、Rust toolchain、npm依存パッケージのダウンロードとRustコンパイルに時間がかかります。ネットワークや組織の端末ポリシーでwingetの導入が止められた場合は、後述の前提条件を手動で準備してください。セットアップスクリプトの内容は[`scripts/setup-and-run.ps1`](scripts/setup-and-run.ps1)で確認できます。

### 手動で準備する場合

次のコンポーネントを用意します。

1. [Visual Studio 2022 Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)の「C++によるデスクトップ開発」workload（MSVC x64/x86 toolsとWindows SDKを含む）
2. [Rustup](https://rustup.rs/)とRustの`stable-x86_64-pc-windows-msvc` toolchain
3. [Node.js LTS](https://nodejs.org/)。Node.js 20.19以降または22.12以降
4. Microsoft Edge WebView2 Evergreen Runtime。Windows 10 version 1803以降とWindows 11では通常OSに含まれます。[公式ダウンロード](https://developer.microsoft.com/microsoft-edge/webview2/)

必要なWindows用Build Tools、WebView2、Rust MSVC toolchainの詳細は[Tauri公式のWindows前提条件](https://v2.tauri.app/start/prerequisites/)を参照してください。

「Developer Command Prompt for VS 2022 (x64)」を開き、リポジトリのディレクトリに移動して以下を実行します。

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
npm ci
npm run tauri -- dev
```

## アプリの使い方

1. [標準入力テンプレート v1](templates/stateflow_input_v1.xlsx)をコピーして、実仕様の機能・状態・入力・イベント・ルールに置き換えます。AC / LINK / HEATERのサンプルデータ入りで、列ヘッダーに説明があります。
2. 右上の「Excel取込」から正式フォーマットのExcelを1件以上選択します。ブックは`Machines`, `States`, `Inputs`, `Outputs`, `Events`, `Transitions`の6シートを使います。
3. 「依存関係・実行順序」で機能ノードを確認し、CURRENT / PREVIOUS / FLOW_ONLY接続を必要に応じて手動で作成します。取込だけで接続線は自動生成されません。ノードの左から右の順が評価順です。
4. 「統合マトリクス」で固定入力、前回出力、前回状態を指定し、イベント行と状態組み合わせのセルを選んで評価トレースを確認します。縦軸はイベント、横軸は各機能の現在状態、セルは次状態です。
5. 「連動レビュー候補」でRustコアが抽出した候補を確認し、設計者が判定とメモを保存します。候補は設計不具合の確定判定ではありません。
6. 右上の「保存」でローカルのSTATEFLOW JSONとして保存し、あとで「開く」から再読込します。各画面のCSV操作でマトリクス結果またはレビュー候補を出力できます。

この評価は全機能の1ステップ確認です。実機の周期、割込、並行動作を再現するものではありません。条件不成立や状態維持だけでデッドロックや設計不具合と断定しません。

## 開発・検証コマンド

MSVC環境が設定されたターミナルで、リポジトリ直下から実行します。

```powershell
npm ci
npm run typecheck
npm test
npm run build
cargo fmt --all -- --check
cargo test -p stateflow-core
cargo clippy -p stateflow-core --all-targets -- -D warnings
npm run tauri -- build
```

`npm run lint`はpackage.jsonにscriptがありません。MSIインストーラーをビルドする端末で`failed to run light.exe`が発生する場合は、Windowsの「オプション機能」からVBScriptを有効にします。詳細な検証環境、結果、未確認項目は[docs/TEST_REPORT.md](docs/TEST_REPORT.md)、画面証跡は[docs/qa](docs/qa/)を参照してください。

## Windows配布物

[GitHub Release v0.0.1](https://github.com/chiguh28/stateflow_verification/releases/tag/v0.0.1)の`STATEFLOW_0.0.1_x64.zip`には次の2ファイルが入っています。

- `stateflow.exe` — アプリ本体
- `STATEFLOW_0.0.1_x64_en-US.msi` — Windows Installerパッケージ

配布ファイルはコード署名されていません。ダウンロード後にWindows SmartScreenの発行元不明の警告が出ることがあります。出所を公式Releaseで確認してから実行してください。署名とSmartScreenについては[Microsoftの説明](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation)を参照してください。

## プロジェクト資料

- [実装仕様書](STATEFLOW_実装指示書.md): 業務ルール、Excel契約、画面仕様、受入条件。
- [実装状況](docs/IMPLEMENTATION_STATUS.md): 完了・未完了フェーズと受入上の制約。
- [AI向け引き継ぎ書](docs/AI_HANDOFF.md): コードマップ、実装済み契約、既知差分、再開手順。
- [技術設計書](docs/ARCHITECTURE.md): Rust / Tauri / Reactの責務、評価・グラフ・Excel・レビューの設計。
- [判断記録](docs/DECISIONS.md) / [検証記録](docs/TEST_REPORT.md)。
- [正式Excel入力テンプレート v1](templates/stateflow_input_v1.xlsx): 6シートと架空の動作例。
