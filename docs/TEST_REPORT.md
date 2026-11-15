# STATEFLOW 検証記録

更新日: 2026-11-16（Asia/Tokyo）

## 実行環境

- Windows上のPowerShell 7.6.9
- Node.js 24.13.1 / npm 11.8.0
- Rust / Cargo 1.87.0、target: stable-x86_64-pc-windows-msvc
- WebView2 Runtime 157.0.4341.46
- 実際のUI表示: 1602×1032
- Windowsエディション、CPU型番、物理RAM容量は未計測。

## 自動検証

| コマンド | 結果 |
| --- | --- |
| `npm run typecheck` | PASS |
| `npm test` | PASS、Vitest 1件 |
| `npm run build` | PASS。Viteから503.02 kBのchunk advisoryあり |
| `npm run lint` | 未実行。`package.json`にlint scriptなし |
| `cargo fmt --all -- --check` | PASS |
| `cargo test -p stateflow-core --target-dir target\core-verify -- --nocapture` | PASS、追加前の実行でunit 11 + integration 10 + lazy scale 1 = 22件 |
| `cargo clippy -p stateflow-core --all-targets --target-dir target\core-verify -- -D warnings` | PASS |
| `cargo check -p stateflow` | PASS |
| `npm run tauri -- build` | PASS、v0.0.1 WindowsリリースアプリとMSI/NSISを生成 |

Cargoのscale test記録: 理論上10,485,760セル。全件要求は50,000セルのAPI上限で拒否し、16セルを6.3521msで取得（今回の再実行値）。RAM使用量は計測していない。

## 正式Excelテンプレートの追加検証（2026-11-16）

- 成果物: `templates/stateflow_input_v1.xlsx`。仕様書4章に定める6シートを含む架空のAC / LINK / HEATER例。
- 回帰テスト: `cargo test -p stateflow-core --test importer_workbooks -- --nocapture` はPASS、3件。`cargo test -p stateflow-core`もPASS、unit 11 + integration 11 + lazy scale 1 = 23件。
- 確認内容: xlsxがRust `calamine`から読めること、3機能・6状態・3入力・1出力・4イベント・6ルール、グラフ辺0本、2種類の未接続CURRENT辺を必要とする4ルール分の診断が出ること。
- 2本のCURRENT辺をテスト内で手動接続したGolden A入力は`AUTO / ON / ON`になる。
- `cargo fmt --all -- --check` と `cargo clippy -p stateflow-core --all-targets -- -D warnings` はPASS。
- `npm run typecheck`, `npm test`（1件）, `npm run build` はPASS。Buildは503.02 kBのchunk size advisoryあり。サンドボックス内でのnpm test/buildはViteのrealpathが`EPERM`となり、承認付き実行でPASSした。
- テンプレートは6シート以外を含まない。手動接続不足は仕様どおりであり、UIでAC→LINKとLINK→HEATERを接続して解析する。

### ゴールデンA〜G

- Golden A: PASS。AUTO_SW OFF→ON、SAFETY_LOCK OFF、HEATER_CMD OFFでAC/LINK/HEATER = AUTO/ON/ON。
- Golden B: PASS。前回出力だけをONに変えるとAUTO/ON/OFFになり、HEATERのPREV.OUTPUT条件が偽で状態を維持した根拠を追跡できる。
- Golden C〜G: PASS。Golden integration test全8件を含むRust core testsが通過。

## 手動UI/リリース検証

- Golden workbookをリリース版へ読み込み、4イベント×8組み合わせ×3機能=96セルを表示。
- 取込直後は接続0本で、AC→LINKおよびLINK→HEATERを手動でCURRENT接続。
- その状態をローカルJSONへ保存し、開き直して2接続を確認。
- 前回出力OFF/ONでGolden A/Bの結果とHEATER条件トレースが切り替わることを確認。
- レビュー候補21件、仕様通り判定とメモの保存/再読込、候補21件CSV出力を確認。
- マトリクス96セルCSV出力を確認。
- マトリクス、レビュー候補、レビュー設定、依存グラフ、ルール一覧、実行設定の6画面を実データで表示。
- 画面証跡: `docs/qa/matrix-golden-a.png`, `matrix-golden-b.png`, `graph-wired-release.png`。
- Excel取込時に手動グラフ接続不足の診断が表示され、配線後に解消することを確認。

## 配布成果物

- EXE: `target/release-v0.0.1/release/stateflow.exe`（11,700,224 bytes、SHA-256 `C014AC447E25EB81170FC90835561943C2EDF4FBBA68617E453FF01E55C427ED`）
- NSIS: `target/release-v0.0.1/release/bundle/nsis/STATEFLOW_0.0.1_x64-setup.exe`（2,740,213 bytes）
- MSI: `target/release-v0.0.1/release/bundle/msi/STATEFLOW_0.0.1_x64_en-US.msi`（4,046,848 bytes）
- 1602×1032のUI手動スモークは版0.1.0のリリースアプリで実施。v0.0.1は版番号変更後のコードを新しいtarget directoryへリビルドし、exe/MSI/NSISを生成した。v0.0.1 exeの再起動スモークはまだ行っていない。
- インストーラー自体の実インストール手順はまだ試していない。

## 未確認・制約

- `package.json`にlint scriptがないため、npm lint相当は実行していない。
- 起動直後にViteのnode_modules realpathがEPERMとなったコマンドがあった。承認された昇格実行ではnpm test/buildとRust formatが通過。初回の拒否はサンドボックス実行制約と判断し、失敗を隠さず記録する。
- Rust fmt check と Clippy も通常のサンドボックス実行では Windowsのpath/config access deniedになり、昇格実行で通過した。Rust test自体は通常実行で通過。
- version 0.0.1への初回Tauriビルドは、起動中の旧exeを上書きできず失敗した。`CARGO_TARGET_DIR=target/release-v0.0.1`の分離ビルドで成功。
- 仕様書指定の1366×768 / 1440×900 / 1920×1080比較、高DPI、キーボード/フォーカス、エラー状態の網羅確認は未実施。
- 複数ブック統合と必須列欠損診断には自動fixture testを追加済み。その他の不正Excelケースの網羅テストは未実施。
- CPUとRAMは測っていないため大規模試験のメモリ量は報告できない。
- 以上により仕様書11章の最終受入は未完了。
