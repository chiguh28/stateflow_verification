# STATEFLOW — Codex / GPT-luna 実装指示書

**版：1.0 / 2026-11-15**  
**種別：実装契約（UI再現仕様 + 機能要件 + データモデル + テスト + 完了基準）**  
**対象：Rust + Tauri 2 + React + TypeScript のローカルデスクトップアプリ**

> **最重要**：本書と同梱の `references/STATEFLOW_UI_Prototype_v6.html` を最初に全て確認すること。UIはユーザーが承認したVer.6を忠実に再現し、ダミーデータ・仮判定を本番解析へ混入させない。途中で独自判断により別デザインへ改変しない。GPT-lunaは段階ごとにテスト・画面確認し、機能を見かけ上動かしただけで完了扱いにしない。

---

## 0. 成果物と合格の定義

### 0.1 何を作るか

複数のExcel状態遷移定義を取り込み、それぞれの機能の**現在状態を直積で全組み合わせ**し、**外部入力1項目の変化**を各組み合わせへ適用して、**全機能を指定順に1回ずつ逐次評価**するソフトウェア。

結果は**縦軸＝イベント変化パターン、横軸＝状態組み合わせごとに機能別の現在状態を独立列で表示したマトリクス**で確認する。各セルの主表示は**その機能の次状態のみ**。連動しない理由を追跡し、仕様レビュー候補を抽出・保存する。

### 0.2 必須の最終成果物

1. 起動可能な **Tauri 2 デスクトップアプリ**（少なくとも開発環境での実行を実証）。
2. React/TypeScriptフロントエンドと、UIから独立してユニットテスト可能なRust解析エンジン。
3. Excel標準入力フォーマットの正式版とサンプル。既存の同梱テンプレートは**旧版参考資料**であり、前回値・イベント・優先順位の扱いを正式版へ補完する。
4. UIの6画面、データの保存・再読込・インポート・入力検証・検証結果表示・CSV出力。
5. 自動テスト、開発/ビルド手順、制約事項、未対応仕様の明示。再現テスト用フィクスチャを付属。
6. 完了時の証拠（テスト結果、実際の画面キャプチャ、動作確認、未完了項目一覧）。

### 0.3 仕様の優先順位

- **ユーザーが確定した業務要件・本書の動作規定** > 動作に関するプロトタイプのダミー挙動。
- **承認済みVer.6の視覚設計・画面構造・操作導線** > 追加デザイン案。
- **正式Excel定義** > 旧テンプレートの不足列・旧サンプル表現。
- 実装上の論理矛盾があれば `docs/DECISIONS.md` に「現象 / 衝突する仕様 / 暫定の安全側判断 / 将来案」を記録する。重大な齟齬は勝手に隠さず報告する。

### 0.4 実装しないこと

- Word/PDF/状態遷移図の直接読み取り。ユーザーがCopilotなどで標準Excelに変換する。
- クラウドアカウント・サーバー・ユーザー登録・ライセンス課金。
- ハードウェア実機制御、CAN通信、周期の実時間シミュレーション。
- 条件未成立＝即バグ/デッドロック、という根拠のない断定。
- AIに条件式を推測補完させる処理、JavaScript `eval` / Python `eval` による条件評価。
- テスト用ダミーの解析を本番プロジェクト読み込み後も表示すること。

---

## 1. 確定した業務要件

### 1.1 入力とモデル

- 複数の標準Excelをインポート可能。単独機能も可。
- 元仕様書は状態遷移図+条件解説、あるいは文章。変換工程はアプリの外。
- 1行が1遷移ルール。AND / OR / NOT / 括弧 / == / != / < / <= / > / >=、機能状態と外部入力の参照を持つ。
- 他機能状態への参照がある。関係ノードはExcelから作るが、**ノード間接続はユーザーが実施**。自動配線禁止。
- 各機能に初期状態がある。無効な初期状態を許可しない。
- 出力/変数更新の詳細仕様は元の状態遷移表とは別資料。最初の版ではこれを勝手に補わない。**前回出力参照**は宣言された信号とユーザーが指定した履歴値で評価できる。

### 1.2 1回の検証の意味

- 全機能の現在状態のCartesian productを理論上生成。各組み合わせは独立試験ケース。
- 行ごとに外部入力の**1項目だけ**を `from_value → to_value` に変化させる。他の外部入力は画面で指定した固定値。
- **全機能を左→右の順に、各1回だけ**評価する。周期やイベントによる実際の起動対象制御はこのモードに含めない。
- 前の機能で更新した `STATE.*` は後の機能から即参照できる。既に評価した機能の再実行はしない。
- 成立した遷移ルールがなければ現在状態を維持（暗黙状態維持）。成立したルールの次状態が同じなら明示状態維持。
- ルール優先順位は**機能内で独立**。同時成立時は priority が小さい方が優先。**機能実行順序（左→右）とは別物**。
- 入力変化行を順番に実行するわけではない。**イベント行相互は独立**し、同じ状態組み合わせの1ステップ試験となる。

### 1.3 履歴参照

- `STATE.AC` = 同じ試験ステップ内の**その時点の最新状態**。
- `BEFORE.STATE.AC` = 今回の試験ステップ**開始時**のAC状態（全状態組み合わせの列から決まる）。
- `PREV.STATE.AC` = **前回確定済みステップ**におけるAC状態。`BEFORE.STATE.AC`とは異なる可能性がある。
- `PREV.OUTPUT.HEATER_CMD` = 前回確定済みステップのヒーター指令の出力値。現ステップの結果から自動的に逆算してはいけない。
- 今回の単独1ステップ・全組み合わせ検証では、PREV値を**独立したテスト条件として手動指定**する。前回値が不明なら `UNKNOWN` として解析不能理由を表示し、勝手に `OFF` / 現在値へ補完しない。
- 将来、実行履歴を連続再生するならステップ完了時に `previous ← completed snapshot` とするが、今期に連続再生は不要。

### 1.4 検出・レビュー

- 状態変化、明示状態維持、暗黙状態維持、ガード不成立、評価不能、優先順位による採用、同順位競合、連動レビュー候補を区別。
- 連動漏れは**設計者確認候補**であり、それだけでは不具合を確定しない。
- 依存関係の上流変化/下流維持、PREV条件による抑止、実行順序による未反映、暗黙保持、全機能状態維持をレビュー抽出ルールとして用意。
- 原因の説明は、参照元、AND/OR条件の真偽（短絡評価で省略しない監査トレース）、採用/不採用ルールと priority、1ステップの実行順序を含む。
- 実際のデッドロック確定には追加の到達可能性/多ステップ探索が必要。**本版では「デッドロック候補」まで**。将来エンジンを拡張する余地は残すが、未実装を検出済みと装わない。

---

## 2. 技術スタックと責務分離

### 2.1 固定スタック

| 層 | 採用技術 | 役割 |
|---|---|---|
| デスクトップ | **Tauri 2** | ウィンドウ・ネイティブダイアログ・ビルド |
| コア | **Rust stable** | Excel取込、DSLパーサ、逐次評価、レビュー抽出、ファイル保存、ページング |
| フロント | **React + TypeScript + Vite** | UI、ビューステート、編集体験 |
| ノードエディタ | **@xyflow/react (React Flow)** | ComfyUI的なカスタムノード/Handles/Edges、ズーム・パン・ドラッグ接続 |
| 大規模マトリクス | **@tanstack/react-virtual** | 縦・横二方向仮想化、固定ヘッダー・スクロール |
| 状態管理 | Zustand等の軽量ストア | UI状態とAPI結果、未保存状態 |
| データ検証 | Rust `serde`/`thiserror` + TS Zod等 | 入力モデルの型安全・分かりやすいエラー |
| Excel読取 | Rust `calamine` | `.xlsx`のセル読み取り、シート単位の入力検証 |
| ローカル保存 | UTF-8 JSONプロジェクト | 解析モデル・配線・設定・レビューの再現可能な保存 |
| テスト | Rust unit/integration/property系、Vitest + Testing Library、Playwright | 解析正しさ、React動作、画面遷移の検証 |

**バージョンは実装時の公式互換性を確認してロックファイルに固定**する。現在存在しない/互換性不明なAPI名を想像して実装しない。

### 2.2 重要なアーキテクチャ制約

1. RustエンジンはTauri依存のないクレート `crates/stateflow-core` に置き、`cargo test` 単独実行可能にする。
2. Tauriラッパー `src-tauri` がRustコアにリクエストを渡し、フロントは型のある `invoke` で通信する。
3. JSX内で状態遷移ルールの真偽を再計算しない。Reactは表示担当。
4. 大容量解析結果を巨大JSONで一括転送しない。必要な**窓/ページ**のみRustに要求する。
5. 全ての入出力・解析・レビューでID、再現条件、モデルrevisionを保存する。
6. UI見本のHTMLを直接 `iframe` で埋め込まない。**Reactコンポーネントとして実装し直す**。原本は比較参照用に `references/` へ保持。
7. オフライン前提。ユーザーの機密仕様書/Excel/解析結果を外部ネットワークに送信しない。テレメトリは初期無効。
8. Tauri capabilitiesは最小権限に限定。パス検証・ファイル上限・不正Excel対策を実装し、ユーザー選択パス以外への不要な読み書きを防ぐ。

### 2.3 推奨ディレクトリ

```text
stateflow/
├─ AGENTS.md
├─ package.json, package-lock.json, vite.config.ts, tsconfig.json
├─ src/
│  ├─ app/                # ルーティング、アプリShell、テーマ
│  ├─ components/         # Button, Card, Panel, Badge, Dialog, SplitPane等
│  ├─ features/
│  │  ├─ projects/
│  │  ├─ import/
│  │  ├─ matrix/
│  │  ├─ review/
│  │  ├─ review-settings/
│  │  ├─ graph/
│  │  ├─ rules/
│  │  └─ execution/
│  ├─ api/                # Tauri API呼出、型定義
│  ├─ stores/
│  └─ styles/
├─ crates/stateflow-core/
│  ├─ src/{model,import,parser,ast,eval,graph,matrix,review,project}/
│  └─ tests/
├─ src-tauri/
│  ├─ src/{commands,state,error}.rs
│  ├─ capabilities/
│  └─ tauri.conf.json
├─ fixtures/              # 有効/不正Excel、期待結果fixture
├─ references/            # 承認済みv6 HTML、旧Excelテンプレ
├─ docs/{DECISIONS,TEST_REPORT,IMPLEMENTATION_STATUS}.md
└─ .github/workflows/
```

---

## 3. UI設計契約 — Ver.6 を忠実に再現する

### 3.1 全体デザイン

- 色調：Tesla風の**ミニマルな白・チャコールブラック・ライトグレー・赤アクセント**。Teslaロゴなどの商標素材は使用しない。
- 色の目安：`--red:#E82127; --ink:#171A20; --bg:#F4F5F6; --line:#E3E5E8; --white:#FFFFFF; --muted:#777D86`。
- 左サイドバーはチャコール、上部バーは白、メインカードは白、装飾は控えめ、角丸約6〜10px。
- 高密度な設計レビュー画面として文字密度と視認性を両立。ダークノードキャンバスはv6のまま維持。
- フォントは環境で利用可能な日本語Sans。読みやすいフォールバック。表内文字切れにはtooltip/詳細を設ける。
- タイポグラフィ・余白・ボーダー・ボタン・サイドバー幅はv6 HTMLからCSSを抽出して可能な限り再現。
- 画面幅1366〜1920px、Windows 100〜150%表示スケーリング、マウス/トラックパッドで破綻しない。

### 3.2 ナビゲーション（同梱v6と同じ6画面）

1. **統合マトリクス** `matrix`
2. **連動レビュー候補** `reviews`
3. **レビュー抽出設定** `review-settings`
4. **依存関係・実行順序** `graph`
5. **遷移ルール** `rules`
6. **実行設定** `settings`

加えて、最小限の「プロジェクト/Excelを開く」「保存」「取込エラー表示」を共通ヘッダーまたはダイアログとして実装。新たな大型ナビ項目の追加は避ける。

### 3.3 統合マトリクス（最優先、画面の顔）

**縦軸**：入力変化の全イベント行（例 `AUTO_SW: OFF→ON`, `AUTO_SW: ON→OFF`, `LINK_SW: OFF→ON`）。  
**横軸**：`組み合わせ01`、`組み合わせ02`…の**グループヘッダー**。各グループ内にAC / LINK / HEATER…の**機能別列**。各列ヘッダーは**現在状態**を表示する。  
**セルの主テキスト**：当該イベント+現在状態の1ステップ評価で得られた**当該機能の次状態**のみ（例 `AUTO`, `OFF`, `ON`）。イベントや矢印・理由をセル内に詰めない。

例：

| イベント（縦軸） | 組01 AC: MANUAL | 組01 LINK: OFF | 組01 HEATER: OFF | 組02 AC: MANUAL | 組02 LINK: ON | 組02 HEATER: OFF |
|---|---|---|---|---|---|---|
| AUTO_SW OFF→ON | AUTO | OFF | OFF | AUTO | ON | ON |
| LINK_SW OFF→ON | MANUAL | ON | OFF | MANUAL | ON | OFF |

※上記の値は**説明用**で、正解はテストfixtureとRust解析から決まる。

- 変更/維持/レビュー候補/評価不能/競合の**色分け**。赤はレビューや競合など本当に注意すべきケースに限定。色だけでなくラベルやアイコン、ARIAなども補助。
- 列グループ境界線、二段ヘッダー、最左イベント列固定、ヘッダー固定、横スクロール、縦スクロール。
- グループ先頭へのジャンプ、特定機能の列表示切替、イベント絞り込み、レビューのみ強調。
- ヘッダーには固定入力値（`INPUT.*`）、履歴値（`PREV.*`）を設定するコントロール。変更すると**Rust計算結果を再取得**する。
- 状態組み合わせの総数・イベント数・検証セル数は、オーバーフローを防止した計算で表示する。
- セル選択時、右詳細パネルに該当機能の**現在→次状態、全機能の逐次評価トレース、ガードの各部分式評価、優先順位、ソースExcel位置、レビュー候補理由**を表示する。
- 選択セルと詳細パネルは、スクロールしても関連が見える。処理中/空データ/エラー/大量件数の状態を美しく表示。
- 大規模データでは**行・列両方を仮想化**して表示セルだけ計算/取得。安易に全セルをDOM/メモリに展開しない。

### 3.4 ComfyUI風 依存関係・実行順序エディタ

**必須**：元v6のキャンバス・ノード・サイドInspector・接続線色・操作感をReact Flowのcustom nodes/edgesで再現。

- インポート後は各機能の**ノードだけ自動生成**し、接続は**0本**から開始。既存プロジェクトを読み込んだ場合のみ保存済み配線を復元。
- ノードをドラッグして配置。パン、ズーム、fit view、ミニマップ（必要なら）、削除確認、接続選択と削除、Undo/Redo。
- ノードの**左側が入力ポート、右側が出力ポート**。右出力→別ノード左入力へドラッグ接続。
- 接続種別：
  - **赤: CURRENT** — `STATE.<source_machine>` の参照を許可 ＋ ソースをターゲットより先に評価。
  - **グレー破線: PREVIOUS** — `PREV.STATE.<source_machine>` または `PREV.OUTPUT.<declared_output>` の参照を許可 ＋ ソースをターゲットより先に評価。線のInspectorでstate/output/信号IDを選択。
  - **白: FLOW_ONLY** — データ参照なし、順序制約のみ。
- 対象ソースIDとターゲットの**具体的なガード参照**を紐付ける。線を引くだけでガード式が自動的に書き換わってはいけない。
- 接続タイプに対応しない参照をガードが使う場合、**未解決参照**としてわかるようにし、検証をブロックする（後述）。
- **左から右が実行順序**。ノード中央のx座標昇順でソート。xが同値/重複なら自動整列または明示エラー。y座標は順序に影響しない。
- 同じノードへの自己配線、逆方向配線、順序の矛盾する移動は禁止。操作を拒否するときはその場で理由を表示。
- 未接続ノードも左→右順で全て1回評価（ユーザー決定済み）。接続線は「データ参照可能性」と「実行順序制約」を表す。
- インポートしたガードから参照要求を**提案として一覧化**してよいが、勝手に接続しない。「未接続/解決済み/参照型不一致」を表示。
- 変更が行われたら解析キャッシュ無効化、マトリクス結果とレビュー件数を自動再計算または再計算ボタンで反映。
- 現仕様では**履歴接続も左→右の順序を伴う**。必要に応じた右→左の時間遅れ依存は将来拡張とし、ユーザー承認済みv6を無断変更しない。

### 3.5 連動レビュー候補一覧

- 他画面から独立したメニュー。「レビュー候補のみ」をリスト表示。
- 表の列：検出ID、イベント変化、元の状態組み合わせ、対象機能、現在→次状態、抽出理由、レビュー判定、元ルールへの参照。
- 検索、抽出理由/イベント/機能/判定によるフィルター、件数表示、ソート、CSV出力、マトリクス該当セルへジャンプ。
- 右詳細ペイン：原因、参照元、評価順序、条件の部分式、PREV値、元Excelソースを提示。
- ユーザーの判定は `未判定 / 仕様通り / 要修正 / 保留`。自由記述メモと日時を保存。案件の安定したfingerprintで再読込後も復元。
- 設定変更で抽出対象が減ってもレビュー判断は直ちに消去しない。非表示・未対象として残す。ソース仕様が変更された場合はfingerprint差異を検出して旧判定を要再確認と表示。

### 3.6 レビュー抽出設定

ルールIDと初期ON/OFFをv6に合わせる。全条件の意味を定義し、条件が成り立たないモデルでも偽陽性を作らない。

| ID | ルール | 初期 |
|---|---|---|
| R01 | 上流状態が変化したが参照する下流が状態維持 | ON |
| R02 | 前回状態/前回出力条件が連動を抑止 | ON |
| R03 | 実行順序の関係で参照元の更新が評価時に未反映 | ON |
| R04 | 暗黙状態維持をすべて抽出 | OFF |
| R05 | 入力を変化させても全機能が状態維持 | OFF |

- 機能・イベントのチェックボックスも持たせる。
- 設定の変更は**解析事実を変更しない**。レビュー候補一覧の対象とマトリクスの強調のみ変える。
- R01は条件抑止による正常な保持も含むため「要レビュー」。
- R02はAND/OR構造を考慮して**PREV参照の値だけを変えた場合に当該ルールの成立可能性が変化する**など、根拠がある場合だけ認定。単にPREVが文中に出るだけで候補にしない。
- R03は適切な順序配線が完成していれば発生しにくい。発生時は「順序定義と参照関係の不整合」を優先して表示。存在しない候補を作らない。
- R04/R05は数が増えがちなのでユーザーが任意で有効化。
- 候補件数は実モデルの解析で算出。プロトタイプの固定 `12件` などは使用禁止。

### 3.7 遷移ルール画面

- 正規化したExcelルールの一覧、検索、フィルター、ガード構文のハイライト、priority、元Excel参照。
- 同時成立したルールの採用根拠を追える。重複/未定義/式パース不能は検証エラーとして表示。
- priorityは必須の非負整数（0以上）とする。低数値が優先。同一優先順位は**異なる次状態が同時に有効になったとき競合**。
- 同じpriority・同じ次状態でも冗長定義の警告を出すが、次状態を勝手にランダム選択しない。
- 行・セル番号など出典位置を保持し、元Excelが無くても画面で確認できる。

### 3.8 実行設定画面

- 左→右のノード位置から生成された順序を**読み取り専用の正**として表示。順序変更の主要操作はノードエディタで行う。
- 上下変更したい場合はグラフ側のx位置変更という**同じドメイン操作**へ変換して同期させる。二重の独立順序設定を持たない。
- 「全機能を必ず1回」「更新済みSTATEを後続へ」「不成立なら状態保持」の確定ルールを明示。
- 設定に未解決参照、重複x、逆向き接続がある場合は検証実行を禁止し、修正導線を出す。

---
## 4. 標準Excel v1（確定フォーマット）

### 4.1 原則

- 同梱の `references/standard_input_template_legacy.xlsx` は、旧テンプレート由来の参照サンプル。現行の必須シート `Outputs` / `Events` と `priority` を追加済みだが、列定義と正式サンプルは `templates/stateflow_input_v1.xlsx` を正とする。
- 複数の `.xlsx` を同時読み込み可能。各ブックには必要なシートだけを配置してもよいが、最終的な統合モデルでは下記のキーが満たされること。
- 全シートで1行目は固定カラム名、2行目以降がデータ。セル結合、1セルに複数ルール、図・色で意味を表す入力は禁止。未定義列は無視ではなく明確に警告（将来拡張を見込む場合は `x_` prefix列だけ許可）。
- ルールの条件文は**Excel計算式ではない**。アプリのDSLとしてパース。空文字や未知参照を曖昧に解釈しない。
- IDはASCII英数字+アンダースコアで固定。表示ラベルは日本語可。参照IDは一意で大文字小文字を区別するか、取り込み時に正規化するかを一貫させる（推奨：大文字IDへ正規化）。

### 4.2 シート定義

#### `Machines` — 機能マスタ

| 列 | 必須 | 例 | 用途 |
|---|---|---|---|
| `machine_id` | YES | `AC` | 一意の機能ID |
| `machine_name` | YES | `空調制御` | 表示名 |
| `initial_state_id` | YES | `MANUAL` | 初期状態 |
| `step_order` | NO | `1` | 初回ノードx配置の参考。取込後はグラフx位置を実行順序の正にする |
| `description` | NO | `空調モード制御` | 説明 |

#### `States` — 状態一覧

| 列 | 必須 | 例 | 用途 |
|---|---|---|---|
| `machine_id` | YES | `AC` | 所属機能 |
| `state_id` | YES | `AUTO` | 状態ID（`machine_id + state_id`で一意） |
| `state_name` | YES | `オート` | 表示名 |

#### `Inputs` — 外部入力

| 列 | 必須 | 例 | 用途 |
|---|---|---|---|
| `input_id` | YES | `AUTO_SW` | 入力ID |
| `input_name` | YES | `AUTOスイッチ` | 表示名 |
| `input_type` | YES | `BOOL / ENUM / NUMBER` | 値型 |
| `allowed_values` | YES | `OFF|ON` / `0..250` | 許容値・範囲 |
| `default_value` | YES | `OFF` | 初期の固定値 |
| `unit` | NO | `km/h` | 単位 |

#### `Outputs` — 前回出力参照のための信号宣言

| 列 | 必須 | 例 | 用途 |
|---|---|---|---|
| `output_id` | YES | `HEATER_CMD` | 出力ID |
| `owner_machine_id` | YES | `HEATER` | 出力を持つ機能 |
| `output_name` | YES | `ヒーター指令` | 表示名 |
| `output_type` | YES | `BOOL / ENUM / NUMBER` | 値型 |
| `allowed_values` | YES | `OFF|ON` | 型ドメイン |
| `default_previous_value` | NO | `OFF` | サンプルのデフォルト前回値。実験設定では明示選択させる |

> 元仕様書から現在出力を更新する処理を取り込んでいないので、`Outputs` は**宣言と前回出力値**の検証用である。`OUTPUT.*` の現在値計算は未サポートなら無効とし、静かに疑似計算しない。

#### `Events` — 検証マトリクスの各行

| 列 | 必須 | 例 | 用途 |
|---|---|---|---|
| `event_id` | YES | `AUTO_SW_UP` | 行ID |
| `event_name` | YES | `AUTOスイッチ ON` | 表示名 |
| `input_id` | YES | `AUTO_SW` | 変化させる入力 |
| `from_value` | YES | `OFF` | 操作前 |
| `to_value` | YES | `ON` | 操作後 |
| `enabled` | YES | `TRUE` | 有効/無効 |

- 1行は1入力だけ変化する。`from_value==to_value` は原則無効とし検証エラー。
- BOOLはテンプレート生成時にON/OFFの両方向行を提案可能。ただし**正規Excelへの書込みはユーザー承認後**。アプリが検証実行時に不意に行を増やさない。
- NUMBERの連続値を無限展開しない。境界入力イベントはExcelに明示する。

#### `Transitions` — 遷移ルールの正

| 列 | 必須 | 例 | 用途 |
|---|---|---|---|
| `rule_id` | YES | `HEATER_001` | 一意ID |
| `machine_id` | YES | `HEATER` | 対象機能 |
| `from_state_id` | YES | `OFF` | 評価前状態 |
| `guard` | YES | `STATE.LINK == ON AND PREV.STATE.AC == AUTO` | DSL条件式 |
| `to_state_id` | YES | `ON` | 次状態 |
| `priority` | YES | `10` | 数値が小さい方が先 |
| `source_ref` | NO | `空調仕様書 3.2 / Sheet2!C17` | 出典 |
| `description` | NO | `連携ON時にヒーター起動` | 補足 |

- この表に**出力更新をでっち上げる列は設けない**。別資料の出力ロジック取込は別フェーズ。
- Excelセル形式の`priority`を数値として検証。空欄のまま取り込まない。
- ルールの`source_ref`に元資料、インポート元のfilename/sheet/row/columnを別途追加メタデータとして格納。

### 4.3 DSLガード式の構文・意味

正式な例：

```text
RISE(AUTO_SW)
FALL(AUTO_SW)
INPUT.SPEED < 30
STATE.AC == AUTO
STATE.AC == AUTO AND (STATE.LINK == ON OR INPUT.MANUAL_ALLOW == ON)
NOT (INPUT.SAFETY_LOCK == ON)
PREV.STATE.AC == MANUAL
PREV.OUTPUT.HEATER_CMD == OFF
BEFORE.STATE.HEATER == OFF
```

**パーサ仕様**：

1. トークナイザで識別子、数値、文字列、括弧、演算子を分ける。`AND`, `OR`, `NOT` は大文字を標準とする。余分な空白を許容。
2. ASTを作成する（正規表現の置換で評価しない）。演算子の優先順位：括弧 > 関数/参照 > 比較 > NOT > AND > OR。比較は非連鎖、`a < b < c` は構文エラー。
3. `RISE(AUTO_SW)` は `AUTO_SW: OFF→ON`（BOOL）だけTRUE。`FALL`はON→OFFだけTRUE。`RISE(INPUT.AUTO_SW)` の別表記を許可するなら同じ意味として正規化。
4. `INPUT.X`はイベント変化後の値を参照。イベント対象以外は固定入力値。
5. `STATE.M`は逐次更新済みの状態、`BEFORE.STATE.M`は今回のステップ開始値、`PREV.STATE.M`は独立履歴値。
6. `PREV.OUTPUT.O`は宣言された出力信号Oの前回値。`O`の所有機能は`Outputs`から解決。
7. ガード式と値の型（BOOL, ENUM, NUMBER）を照合。型不一致・参照先未定義・関数への不正型・未知演算子は診断付きエラー。
8. 未指定の履歴値等は`UNKNOWN`。論理演算は3値論理（TRUE/FALSE/UNKNOWN）で、例えば `FALSE AND UNKNOWN = FALSE`, `TRUE OR UNKNOWN = TRUE`, `TRUE AND UNKNOWN = UNKNOWN`。比較にUNKNOWNが関わればUNKNOWN。
9. エラーではないUNKNOWNと、入力形式エラーを区別する。可視化トレースに各葉の値、部分式の結果、AST位置を記録する。
10. 将来拡張用にAST enumをシリアライズ可能とし、重複パースを避ける。

> 「前回」は今回の機能実行順序で一つ前に評価した機能を意味しない。「前回確定ステップ」のスナップショットである。この定義をコード、ヘルプ、テストで統一する。

---

## 5. ドメインモデルとプロジェクト保存

### 5.1 型設計（概念）

```rust
struct Project {
  schema_version: u32,
  project_id: String,
  name: String,
  machines: Vec<Machine>,
  states: Vec<State>,
  inputs: Vec<InputDef>,
  outputs: Vec<OutputDef>,
  events: Vec<EventDef>,
  transitions: Vec<TransitionRule>,
  graph_nodes: Vec<GraphNode>,
  graph_edges: Vec<GraphEdge>,
  review_settings: ReviewSettings,
  review_decisions: Vec<ReviewDecision>,
  scenario: Scenario,
  imports: Vec<ImportSource>,
  revision: u64,
}

struct GraphNode { machine_id: String, x: f64, y: f64 }
enum EdgeKind { CurrentState, PreviousState, PreviousOutput, FlowOnly }
struct GraphEdge {
  edge_id: String,
  source_machine: String,
  target_machine: String,
  kind: EdgeKind,
  output_id: Option<String>,
  target_reference: Option<String>,
}
struct Scenario {
  fixed_inputs: Map<InputId, TypedValue>,
  previous_states: Map<MachineId, Option<StateId>>,
  previous_outputs: Map<OutputId, Option<TypedValue>>,
}
struct CellResult {
  event_id: String,
  combination_id: String,
  machine_id: String,
  initial_state_id: String,
  next_state_id: Option<String>,
  status: CellStatus,
  selected_rule_id: Option<String>,
  trace_id: String,
}
```

**このコードは型の契約例**。具体的なRust実装で型安全なnewtype ID、enum、serde、エラー型に置換してよい。

### 5.2 保存

- 保存形式はローカルの **`.stateflow.json`**（`schema_version`必須）。ユーザーが場所を選択して保存・再読込できる。
- インポート時点の**正規化した状態/ルール/入力/イベント定義**をプロジェクトに格納。元Excelが移動/削除されても解析自体は再現できる。
- 参照用の元ファイル名、sheet、row、source_ref、元ファイルhashを保持。元ファイルが存在しなければソースファイル直接表示だけ無効化。
- グラフノード位置、接続線の種類と参照、レビュー抽出条件、固定入力、PREV値、レビュー判定・メモを保持。
- 保存は一時ファイル＋atomic rename（可能なOS）を基本にし、中途半端なJSONで破損しないようにする。読込時はスキーマ検証。破損/新バージョンには説明付きエラー。
- 自動保存する場合もバックアップ/復帰を意識。Unsaved indicatorと終了確認を付ける。
- importで既存machine_idが衝突した際は、無言の上書き禁止。「上書き/別ID/中止」の選択か、検証不能エラーにする。
- 新しいExcelを再インポートすると既存の配線とレビュー判定を保持できるかID差分をチェックする。消滅した機能や信号に対するエッジは、黙って削除せず未解決として表示。

### 5.3 レビューIDの安定性

- 候補IDは**プロジェクトID、イベントID、元状態組み合わせの各machine state、対象machine、抽出ルールID、正規化した条件/モデルrevision要素**から決定的に生成。
- レビューの判断を必要以上に消さない。仕様変更により判定根拠が変わったら `needs_recheck=true` にする。
- 並び順、スクロール順、日時だけでreview IDを作らない。

---

## 6. Rust解析エンジン — 処理順序とエラー意味論

### 6.1 インポート→検証→解析

```text
load Excel files
  -> normalize sheets/columns/types
  -> validate IDs, states, references, priority, events
  -> parse guard DSL -> AST
  -> resolve guard reference requirements
  -> create nodes ONLY (edges = [])
  -> user draws edges and configures previous source bindings
  -> validate graph / unresolved references / scenario inputs
  -> determine execution order from node x positions
  -> per event-row + per combination evaluate each machine ONCE
  -> compute cell results + detailed trace
  -> derive review findings from trace + settings
  -> virtualized matrix and review list
```

### 6.2 依存関係と配線の意味

- 条件式中の `STATE.AC` が他機能Bのルールにある場合、Bへ入る `AC→B` **CurrentState** edgeが必要。
- `PREV.STATE.AC`の場合、`AC→B` **PreviousState** edgeが必要。`PREV.OUTPUT.HEATER_CMD`の場合、所有機能 `HEATER→B` の **PreviousOutput** edgeが必要。
- 同一機能の内部状態参照は自己接続不要。
- Excelからの参照依存関係は「**接続候補一覧**」にだけ出す。接続線を自動で追加しない。
- 配線済みでも対応するガード参照を検証し、ソース/型/信号の誤接続を検知する。
- **未解決の他機能参照がある場合、全体解析を成功扱いで実行しない**。解決待ちと対象ルールを表示する。フロー線だけではデータ参照を満たせない。
- 同じソース→ターゲットに複数の種類のedgeがあることは許容（CURRENT + PREVなど）。完全重複線は警告。
- 描画上「左から右」以外の向きは許可しない。保存/再読込時も再検証。

### 6.3 逐次評価の正式アルゴリズム

入力：一つの `EventDef e`、一つの状態組み合わせ `S0`、固定入力 `F`、前回状態・出力 `P`、有効なグラフと全ルール。

```text
current := clone(S0)
before := immutable clone(S0)
previous := immutable clone(P)
inputs_before := clone(F); inputs_before[e.input] := e.from
inputs_after  := clone(F); inputs_after[e.input]  := e.to
trace := []

for machine in nodes sorted by x ascending:
    state_before_machine := current[machine]
    candidates := transition rules where
          rule.machine == machine
          AND rule.from_state == state_before_machine
    for rule in candidates:
          evaluate guard AST using current,before,previous,inputs_before,inputs_after
          store leaf and sub-expression truth trace
    eligible_true := candidates with guard == TRUE
    eligible_unknown := candidates with guard == UNKNOWN
    if an UNKNOWN higher/equal-priority rule could change chosen result:
          status := INDETERMINATE; preserve unresolved alternatives
          do NOT quietly choose lower-priority rule or invent a definitive next state
    else if eligible_true is empty:
          current[machine] := state_before_machine
          status := IMPLICIT_HOLD
    else:
          best_priority := min(eligible_true.priority)
          best := eligible_true where priority == best_priority
          if best has >1 DISTINCT to_state:
              status := CONFLICT; do NOT invent a winner
          else:
              chosen := deterministic best rule (same-destination ties logged as redundant)
              current[machine] := chosen.to_state
              status := CHANGED or EXPLICIT_HOLD
    append trace with candidates, guard truth, selected rule, reason

return current+trace+cell status; errors never disguised as HOLD
```

- **重要**：未知状態・優先順位競合が出た時点で下流の依存状態を一意に決められない。下流は`UNKNOWN`伝播または独立候補分岐として扱い、架空の単一結果で上書きしない。MVPでは`判定不能`で停止/部分トレースを返してよいが、理由を明示する。
- 次状態が同じでもguardが成立した場合は `EXPLICIT_HOLD`、一致する遷移が1件もなければ `IMPLICIT_HOLD`。
- 既存の入力・前回情報が不正であれば、行を赤く塗るのではなく専用の診断として提示する。
- 評価中の出力信号の現在更新は未定義。前回出力は読取専用。
- 矛盾したガードなどは読み込み時の静的診断にも出せるが、本番の正誤判定はモデルが指定する前提に依存する。

### 6.4 解析結果の品質分類

| 検証結果 | 意味 | 見せ方 |
|---|---|---|
| `CHANGED` | 次状態が変化した | 変化色/差分 |
| `EXPLICIT_HOLD` | 条件成立・同状態へ遷移 | ほぼ中立色 |
| `IMPLICIT_HOLD` | どのルールも成立せず維持 | 中立色（設定でレビュー強調可） |
| `REVIEW_CANDIDATE` | 指定レビュー条件に該当 | 赤淡色/要確認ラベル |
| `PRIORITY_SELECTED` | 複数成立だが優先順位により一意 | トレースに明示（誤検出しない） |
| `CONFLICT` | 同順位で相異なる次状態が成立 | 強いエラー表示/判定不能 |
| `UNKNOWN` | 欠落/不確定な前回値などにより一意に決まらない | 保留/未設定の案内 |
| `INVALID_MODEL` | スキーマ・接続・型不正 | 解析実行を止めた入力エラー |

マトリクスの「レビュー候補」の視覚強調は基礎結果の上に**オーバーレイ**する。状態自体の真偽をレビュー設定で変更してはならない。

### 6.5 重要なロジックの境界ケース

- `A AND NOT A` は成立しない。存在しない遷移として状態維持。構文としては合法。
- `A AND (B OR C)` と `(A AND B) OR C` は異なる評価をする。
- higher priority TRUE + lower priority TRUE → higherのみ採用、競合としない。
- same priority TRUEの異なる次状態 → CONFLICT。
- `PREV.STATE.AC` と `BEFORE.STATE.AC` が違っても各々正しく評価。
- 今回のイベントで現在状態が変わっても `PREV.STATE` は変化しない。
- 下流機能が上流の更新済み `STATE` を参照し、対応するCURRENT接続が成立していれば1ステップ内で連動。
- 元状態の直積には到達不能な組み合わせが含まれ得る。**直積生成＝実機で到達可能という主張ではない**。到達可能性は将来別検証とする。
- 条件不成立や1ステップ状態維持だけでデッドロックを断定しない。

---

## 7. 大規模マトリクスの計算・表示方式

### 7.1 計算量

状態組み合わせ数 `C = ∏ |States(machine_i)|`。イベント数 `E`、機能数 `M` のとき表示セル数は `E × C × M`。

例：3機能×2状態 → C=8、E=5 → 120セル。  
例：8機能×4状態 → C=65,536、E=20 → 10,485,760セル。  

**絶対禁止**：全セルを同時にReactで描画したり、全組み合わせの全結果を一括でTauri IPCへ返したりすること。

### 7.2 窓取得API

Rust側で「event index」「combination index」を遅延に展開し、必要な部分だけ評価する。

- `count_combinations(project)` はchecked整数（`u128`程度）で計算し、オーバーフロー/上限を明示。JS側へ安全な整数範囲を超える値は文字列で渡す。
- `get_matrix_window({event_start, event_count, combination_start, combination_count, revision})` で必要な窓を取得。窓セル数を制限し、キャンセル/世代番号で古いリクエスト結果を破棄。
- `get_cell_trace(event_id, combination_id, machine_id, revision)` で完全な詳細トレースを別取得。
- `get_review_page({filters, offset_or_cursor, limit, revision})` で候補一覧をページング。
- 状態組み合わせ列はx順に機能配列を固定し、**右端機能が最速で変化**する決定的な混合基数インデックスで復元する。各combo IDは安定させる。
- 演算済みASTとリファレンス解決情報をキャッシュ。モデル、配線、固定入力、PREV値、実行順序のrevision変更で無効化。
- CSVのフルエクスポートはRust側で**ストリーミング書出し**を行い、件数確認・進捗・中断を提供。巨大CSVでディスクを埋めないよう予想容量を表示。

### 7.3 UX性能の受入目標（目安、環境情報と実測値を必ず報告）

- 本番ビルドの標準的なデスクトップで、3×2状態・5イベントのサンプル読み込み/検証は通常操作で待たされない。
- 8機能×4状態・20イベントでも画面は**仮想化により操作可能**で、初期表示のために1千万セル全生成しない。
- スクロール中のDOMノードは可視窓＋overscan程度に制限。目標は明確なフリーズ・白画面がないこと。
- 各測定値を `docs/TEST_REPORT.md` にCPU、RAM、OS、データ規模とともに残す。無根拠な「60fps保証」等の宣言は禁止。

---

## 8. 最低限のTauriコマンド/API契約

フロントが独自ロジックを持たず、型生成/共通DTOを使えるようにする。名称は推奨で、整合が取れていれば変更可。

| コマンド | 入力 | 出力/処理 |
|---|---|---|
| `import_excel_files` | ファイルパス配列 | 取込/検証結果、未解決参照一覧、ノード一覧 |
| `validate_project` | project revision | 診断（エラー/警告/参照要求） |
| `get_project` | なし/ID | 保存済みプロジェクトモデル |
| `save_project` | path/project | 検証後安全にローカル保存 |
| `open_project` | path | JSONスキーマ移行・正規化・読込 |
| `update_graph` | nodes/edges + base revision | 接続バリデーション・新revision・実行順序 |
| `update_scenario` | fixed inputs/previous states/outputs | 新revision・未定義項目診断 |
| `get_matrix_metadata` | revision | 行/列/組合せ数、イベント・機能一覧 |
| `get_matrix_window` | event/combo範囲 + revision | 次状態の仮想窓、結果種別 |
| `get_cell_trace` | event,combo,machine,revision | ルール/論理評価/順序の完全記録 |
| `get_review_page` | rule/decision/keyword filters + revision | 候補と件数 |
| `update_review_settings` | settings + revision | 再抽出結果/新revision |
| `save_review_decision` | candidate ID, judgment, note | 保存完了結果 |
| `export_reviews_csv` | save path + filter | CSV出力と件数 |
| `export_matrix_csv` | save path + scope | 大容量制限と進捗つきCSV |

- Tauri `invoke` は型検証を通す。結果エラーは `code`, `message`, `detail`, `sourceLocation`を持つ構造体として返す。
- 長時間処理はUIスレッドをブロックしない。キャンセルを可能にし、古いリクエストを無効にする。
- ファイル選択・保存はネイティブダイアログを使う。可能な限りパスと入力内容をRust側で再検証する。
- 読込サイズ、シート数、ルール数の上限は設定化し、超過時に安全なエラーメッセージを返す。

---
## 9. 必須テストフィクスチャと期待結果

### 9.1 ゴールデンモデル「空調→連携→ヒーター」

`Machines`: AC=MANUAL/AUTO、LINK=OFF/ON、HEATER=OFF/ON。ノードx順は `AC → LINK → HEATER`。

`Inputs`: AUTO_SW=OFF/ON、LINK_ENABLE=OFF/ON（固定ON）、SAFETY_LOCK=OFF/ON（固定OFF）。

`Outputs`: HEATER_CMD=OFF/ON（所属HEATER）。

`Events`: `AUTO_SW OFF→ON`、`AUTO_SW ON→OFF`、`LINK_ENABLE OFF→ON`、`LINK_ENABLE ON→OFF`。

主要なルール：

```text
AC    MANUAL, RISE(AUTO_SW)                                                   -> AUTO,    priority=10
AC    AUTO,   FALL(AUTO_SW)                                                   -> MANUAL,  priority=10
LINK  OFF,    STATE.AC == AUTO AND INPUT.LINK_ENABLE == ON                     -> ON,      priority=10
LINK  ON,     STATE.AC == MANUAL OR INPUT.LINK_ENABLE == OFF                   -> OFF,     priority=10
HEATER OFF,   STATE.LINK == ON AND PREV.OUTPUT.HEATER_CMD == OFF
              AND INPUT.SAFETY_LOCK == OFF                                     -> ON,      priority=20
HEATER ON,    STATE.LINK == OFF OR INPUT.SAFETY_LOCK == ON                      -> OFF,     priority=10
```

必要な**ユーザー手動接続**： `AC → LINK (CURRENT)`, `LINK → HEATER (CURRENT)`。`HEATER_CMD`はHEATER自己参照のPREV出力として扱うため、別ノードへの接続は不要。

**正解テストA**：イベント=`AUTO_SW OFF→ON`、現在組合せ `AC=MANUAL, LINK=OFF, HEATER=OFF`、前回出力 `HEATER_CMD=OFF`、固定 `LINK_ENABLE=ON, SAFETY_LOCK=OFF` なら、次状態は **`AC=AUTO, LINK=ON, HEATER=ON`**。評価順トレースはAC→LINK→HEATER。

**正解テストB**：Aと同じ条件で**前回出力のみ `HEATER_CMD=ON`**にすると、次状態は **`AC=AUTO, LINK=ON, HEATER=OFF`**。ヒーターのガードにあるPREV条件がFALSEであり、R02レビュー候補になる根拠を出す。

**正解テストC**：Aと同じ現在組合せで `LINK_ENABLE=OFF` なら、次状態は **`AC=AUTO, LINK=OFF, HEATER=OFF`**。この保持は条件で説明できる。レビューの「不具合確定」とはしない。

**正解テストD**：Aのグラフ接続が存在しない場合、**未解決の他機能参照**としてエラー。正解らしく見える疑似結果を返してはならない。

**正解テストE**：同一機能/同一from state/同じpriorityで2ルールTRUE・行先が異なる場合は **CONFLICT**。優先順位が異なる場合は小さいpriorityを採用。

**正解テストF**：`PREV.STATE.AC`と`BEFORE.STATE.AC`を異なる値にした独立試験で、当該ガードの判定が正確に異なる。

**正解テストG**：マトリクスの全イベント行を計算しても、各行を**連続した次ステップとして順送りしない**。毎行同じテスト前提の独立検証。

### 9.2 パーサ・型・論理評価

以下は少なくとも自動テスト化：

- 括弧優先順位、`NOT`、AND/ORの各真理値表、3値論理のUNKNOWN伝播。
- ENUM同士の一致、NUMBER比較、負数・小数の境界、型不一致、未定義機能/状態/入力/出力。
- `RISE/FALL`の旧値・新値、変化しないイベント、別入力の変化、不正な入力from/to。
- 同じpriority/異なるpriorityの多重成立、同じ行先への冗長定義、ガードなしの暗黙保持。
- PREVとBEFOREの違い、自己参照、他機能参照、現在出力を勝手に推定しない。
- 無効Excel（欠損シート、重複ID、空priority、不正guard、不明source、存在しない初期状態）の場所付き診断。
- 接続0本から開始、自動配線禁止、CURRENT/PREV/OUT/FLOWの相互非互換、右→左禁止。
- 優先順位とノードのx並び替え後の**解析結果の差**を回帰テスト。
- 保存→再起動→再読込で配線、PREV条件、判定メモ、固定入力、抽出条件が一致。

### 9.3 UI / E2E

**Playwright（Web版UI + モックのRust APIレスポンス）**で少なくとも：

1. プロジェクトを開く/未保存表示/保存の流れ。
2. 6画面の切替と現在ページ表示。
3. 複数イベント×グループ列のマトリクス表示、セル選択、トレース表示。
4. 横/縦スクロールでイベント列とグループヘッダーが破綻しない。
5. フィルターでレビュー候補数だけが変わり、計算結果そのものは不変。
6. ノードの現在・前回・フロー接続、取り消し、移動、左→右順検証。
7. 同じ入力条件を再入力しても結果が安定する。
8. レビュー判定・コメントの保存とCSVエクスポート。

**Rust統合テスト**では、実xlsxを読み込んだゴールデンモデルの正解A〜Gを検証。Webのモックだけで「アプリ完成」と判断しない。

### 9.4 診断の可観測性

- 解析・取込・保存のエラーはUIに日本語の原因、対象Excelのsheet/row、修正方針を表示。
- 起動時の問題、Rust panic、IPCエラー、ファイル読み書き失敗をログへ。機密Excelの本文を大量にログ出力しない。
- 未解決参照/重複ルール/UNKNOWN/CONFLICTは区別し、検証不能の原因を隠さない。

---

## 10. Codex / GPT-luna に実行させる実装フェーズ

> この順序を守る。各フェーズの終わりで `docs/IMPLEMENTATION_STATUS.md` と `docs/TEST_REPORT.md` を更新。次へ進むためのゲートを満たすまでは、外観だけ見せて完了扱いしない。

### Phase 0 — リファレンス監査と計画

**作業**

- `references/STATEFLOW_UI_Prototype_v6.html` を読み、ナビゲーション、DOM要素、画面構成、配色、操作を調査する。
- 実装要件とデモ仕様の差分を一覧にする。対象サンプルデータ・前回情報・レビュー抽出・ComfyUI風ノードの振る舞いを記録。
- リポジトリ構成、型モデル、APIリスト、フェーズ・タスクチェックリストを作る。
- 開発マシンのOS、Rust toolchain、Nodeバージョン、Tauri 2要件、WebViewを確認。

**完了ゲート**： `docs/IMPLEMENTATION_STATUS.md` に「承認済みUIの6画面と各操作」を完全列挙し、実装時の不明点を記録。未確認を勝手に仮定しない。

### Phase 1 — Tauri起動とUI骨格

**作業**

- Tauri 2 + Vite + React + TSを初期化。
- v6のデザイン変数、AppShell、6画面、共通ボタン、カード、テーブル、split view、詳細ペインをコンポーネント化。
- ノード/マトリクスには**Phase 1だけ固定サンプル**を使ってよいが、別モジュール `fixtures/uiDemo.ts` へ隔離。`production mode`の実データ経路へ紛れ込ませない。
- 見た目の一致をスクリーンショットで比較。最低1366×768、1440×900、1920×1080。

**完了ゲート**：Tauriが起動し、6画面が切替わり、Ver.6の基本レイアウトと配色が大きく崩れない。ハードコードされたダミー計算がRustエンジンへ混入していない。

### Phase 2 — Excel / DSL / 正規化モデル

**作業**

- `Machines, States, Inputs, Outputs, Events, Transitions` の標準Excel v1テンプレートと正例/負例を作成。
- 複数Excelの取り込み、ID/型/参照/priority検証、ASTパース、元ファイル出典トレース。
- 取込時は機能ノードのみ生成、エッジ0本。
- 読込診断の一覧、修正待ち表示。旧テンプレートを取り込んだ場合は不足列を具体的に提示する。

**完了ゲート**：実xlsxの正例・負例のRust自動テストを通過。未知の条件をTRUE/FALSEへ適当に丸めない。

### Phase 3 — ComfyUI風ノード接続＋実行順序

**作業**

- React Flowでv6のノードエディタを再実装。
- 3種類の線、前回線のstate/output選択、接続Inspector、プレビュー、削除、x順序、エラーハンドリング。
- guard ASTの依存参照要求と、ユーザー接続の整合性を検証。
- ノードを並び替えたらRustへ実行順序を同期し、未解決がある状態で誤解析しない。

**完了ゲート**：出力ポートから入力へ接続でき、再読込で復元、誤接続が拒否され、左→右順を常に保つ。元v6の操作感と一致。

### Phase 4 — 1ステップRust解析エンジン

**作業**

- 6.3の厳密アルゴリズムを実装。ガードASTを評価し、priority、暗黙/明示保持、前回値、UNKNOWN、CONFLICTを判別。
- 機能をx昇順で各1回のみ評価。各イベント行は独立試験。
- データ依存は手動配線を解決できるまで検証しない。
- 真理値の枝ごとの評価と採用/不採用の理由をRustトレースへ記録。

**完了ゲート**：ゴールデンテストA〜Gの合格、競合/前回履歴/同一ステップ逐次更新のユニットテスト。

### Phase 5 — マトリクス実結合・大規模データ対策

**作業**

- UIダミー経路を置き換え、実プロジェクトのRust解析結果を表示。
- 行・列仮想化、固定イベント列/二段ヘッダー、窓API、選択セル詳細。
- 操作/設定変更時のキャッシュ無効化、古いIPC結果の破棄、ロード/キャンセル。
- 10万〜1000万級「理論セル数」のストレスケースで全量描画・転送を避ける。

**完了ゲート**：現実にインポートしたExcelがUIで解析される。イベント数・組み合わせ数・機能数に応じて表が生成され、次状態表示とトレースが一致。

### Phase 6 — レビュー抽出 / 判定 / エクスポート

**作業**

- R01〜R05、機能・イベントフィルター、レビュー候補一覧、判定とメモ、再審査フラグ。
- マトリクスの要確認強調とレビュー候補件数を同一データソースにする。
- レビューCSV（日本語Excelで開けるUTF-8 BOM等の扱いを検証）。全マトリクスCSVはストリーミング。

**完了ゲート**：レビュー条件ON/OFFで候補が正しく増減し、基礎遷移結果は不変。判定保存→再読込で復元。

### Phase 7 — アプリの磨き込み / QA / 納品

**作業**

- すべてのUI操作にエラー、空状態、処理中、無効操作、確認モーダルを用意。
- キーボード操作、フォーカス、コントラスト、日本語折返し、高DPI、横スクロール、ズームで破綻がないか確認。
- Windowsで最低限のパッケージング確認。macOSはビルド可能な環境がある場合に確認し、未実行なら明記。
- lint, typecheck, Rust test/clippy, frontend tests, frontend build, Tauri buildを実行。
- README、設定方法、Excelフォーマット、操作マニュアル、既知の制約、テストレポートを整備。

**完了ゲート**：下記11章に全項目合格。未完了や環境制約を隠さない。

---

## 11. 納品前の受入チェック（1件でも未達なら「完成」と言わない）

### UI忠実度

- [ ] 6画面のナビ、メニュー名、デザイン、色、余白、主要UIの導線がv6と概ね一致。
- [ ] マトリクスは**縦イベント、横に現在状態の機能別列、セルは次状態のみ**。
- [ ] 詳細パネルは独立し、選択セルでガード評価と全機能トレースを確認できる。
- [ ] ノードはComfyUI風にドラッグ、ポートから接続・削除、状態/前回/順序の3線種を操作可能。
- [ ] 前回状態と前回出力は接続Inspectorで区別され、保存・再読込できる。
- [ ] ルール一覧・レビュー一覧・レビュー設定・実行設定の操作が形だけでなく永続化される。

### 解析機能

- [ ] 複数Excelから正規モデルを生成し、取込エラーに元の行列情報がある。
- [ ] 各機能の初期状態・全状態・イベント・priority・guardの評価が正確。
- [ ] インポート直後はノードのみで、配線は0本。未接続の依存を検知できる。
- [ ] 実行順序は左→右、各機能1回、後続は更新済み現在状態を参照。
- [ ] PREV/BEFORE/STATEを混同せず、前回値を未設定なら補完せず保留。
- [ ] AND/OR/NOT、関数、比較、優先順位、暗黙/明示保持、同順位競合のテストが合格。
- [ ] ゴールデンモデルA〜GがRustテストで合格。
- [ ] モデルの意味を推定するLLMや文字列evalを解析経路で使用していない。

### 運用機能

- [ ] プロジェクト保存/再読込で全モデル・配線・固定入力・PREV情報・判定結果が保持。
- [ ] レビュー抽出ルールの切替は基本の解析値を変えず、候補・強調のみ変更。
- [ ] レビューのCSV出力と案件の特定、元Excel参照が可能。
- [ ] 大規模な理論上の直積でも全件DOM描画せず、無制限のメモリ使用を避ける。
- [ ] 実機の周期を模擬しない「全機能1ステップ検証」であることをUIに明記。
- [ ] 条件不成立を「デッドロック確定」と断定していない。
- [ ] テスト/型チェック/ビルドの実行ログと環境を残している。

### 最終報告の必須フォーマット

```text
1. 実装完了した機能と、画面ごとの主要操作
2. スクリーンショット/ビルド/実行ファイルの場所
3. npm test / typecheck / lint / build、cargo test / clippy / tauri build の実測結果
4. ゴールデンモデルA〜Gの合格状況
5. 大量データ試験の規模・時間・RAM・環境
6. 未実装項目、仕様上の仮定、既知の問題
7. 次の実装で優先すべき事項（最大3件）
```

**禁止**：未実行を「合格済み」、ダミーデータを「実Excel解析済み」、画像だけのUIを「操作可能」と表現すること。

---

## 12. リスク・判定保留・拡張の境界

1. **状態組み合わせ数の指数爆発**：内部は遅延生成・分割取得。負荷が大きい場合は対象イベント/機能/組合せで制限。サイレントに切り捨てない。
2. **前回状態の曖昧さ**：1ステップの直積表だけでは履歴は定まらない。手動設定値（シナリオ）を検証入力とする。自動推定禁止。
3. **連動の正当性**：依存関係があることだけでは、「連動するのが正しい」とは証明できない。レビュー候補のまま。
4. **グラフと条件式**：ノード接続は入出力と実行順序を担う。条件式に対応する参照の未解決は実行不能として知らせる。
5. **全機能1回評価は実機の周期処理と一致しない場合がある**：シミュレーション結果を実機の時系列保証と表現しない。
6. **同時・並列処理、割込、信号更新タイミング**：ユーザー決定により現版は左→右逐次。実システムの高精度検証は後続フェーズ。
7. **Excelの誤変換**：Copilotの作ったExcelは機械的に妥当でも原仕様と異なる可能性。`source_ref` と取込プレビューを残し、人が確認する。
8. **文字列DSLの将来互換**：schema_versionで管理。構文追加時は後方互換/明示移行。
9. **過去のデモ試算値**：v6の12件等は受入期待値ではない。新しいRustモデルから再計算する。

---

## 13. 開発時の品質規律（GPT-luna向け）

- 一度に全機能を書き上げようとせず、**小さく動かして段階ごとに検証**する。
- コード変更前に関連ファイルとテストを読む。既存の動くUIや他画面を壊さない。
- 自己評価・不明点・実装上の仮定を `docs/DECISIONS.md` へ書く。要件の無断簡略化をしない。
- 解析エンジンの変更には、最低1つの**失敗を再現するテスト**を追加してから修正する。
- 画面の見た目については実際にブラウザ/Tauriで表示してスクリーンショット比較。コードだけを見て忠実度を判断しない。
- フロントに `any` を乱用しない。API型とRust DTOの不一致をビルドで検知できるようにする。
- 本質的な未実装をスタブで埋めて完了扱いしない。「準備中」を正しく示し、ゲートは未達とする。
- 変換Excelを外部クラウドへ自動送信しない。機密入力をサンプルに無断流用しない。
- 指示が実装不可能/矛盾した場合は、理由と代替案を提示する。軽微なライブラリ選択は自律的に決め、記録する。

---

## 14. 参照公式ドキュメント（実装前に最新版を確認）

- Tauri 2: https://v2.tauri.app/start/
- Tauriアーキテクチャ: https://v2.tauri.app/concept/architecture/
- Tauri Dialog: https://v2.tauri.app/plugin/dialog/
- Tauri Security Capabilities: https://v2.tauri.app/security/capabilities/
- React Flow custom nodes: https://reactflow.dev/learn/customization/custom-nodes
- React Flow Handles: https://reactflow.dev/learn/customization/handles
- TanStack Table virtualization: https://tanstack.com/table/latest/docs/framework/react/guide/virtualization
- TanStack Virtual: https://tanstack.com/virtual/latest/docs/introduction
- Rust calamine: https://docs.rs/calamine/latest/calamine/
- Rust rusqlite（将来DBが必要になった場合のみ）: https://docs.rs/rusqlite/latest/rusqlite/

**最後に**：目的は、単なる綺麗なノードUIや巨大表の生成ではない。**「1つのスイッチ操作で複数機能がどの条件で連動/保持したのか」を全状態組合せに対して機械的かつ説明可能な形で可視化し、仕様レビューを支援する**こと。UIの忠実さと検証ロジックの正確さを両方満たして、初めて完成とする。
