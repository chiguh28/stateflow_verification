import { invoke } from '@tauri-apps/api/core';

export type ValueType = 'boolean' | 'enum' | 'number' | 'string';

export type ValueDomain =
  | { kind: 'boolean' }
  | { kind: 'enum'; values: string[] }
  | { kind: 'number'; values: { minimum: number | null; maximum: number | null } }
  | { kind: 'string' };

export type TypedValue =
  | { type: 'boolean'; value: boolean }
  | { type: 'enum'; value: string }
  | { type: 'number'; value: number }
  | { type: 'string'; value: string }
  | { type: 'unknown' };

export type Machine = {
  id: string;
  name: string;
  initial_state: string;
  description: string | null;
};

export type StateDefinition = { machine_id: string; id: string; name: string };
export type InputDefinition = {
  id: string;
  name: string;
  value_type: ValueType;
  domain: ValueDomain;
  default_value: TypedValue;
  unit: string | null;
};
export type OutputDefinition = {
  id: string;
  owner_machine_id: string;
  name: string;
  value_type: ValueType;
  domain: ValueDomain;
  default_previous_value: TypedValue | null;
};
export type EventDefinition = {
  id: string;
  name: string;
  input_id: string;
  from_value: TypedValue;
  to_value: TypedValue;
  enabled: boolean;
  source: SourceLocation | null;
};
export type SourceLocation = { file_name: string; sheet: string; row: number; column: string | null };
export type TransitionRule = {
  id: string;
  machine_id: string;
  from_state: string;
  guard_source: string;
  guard: GuardExpr | null;
  to_state: string;
  priority: number;
  source_ref: string | null;
  source: SourceLocation | null;
  description: string | null;
};
export type GuardExpr = { kind: string; value?: unknown };
export type GraphNodeDto = { machine_id: string; x: number; y: number };
export type EdgeKind = 'current_state' | 'previous_state' | 'previous_output' | 'flow_only';
export type GraphEdgeDto = {
  edge_id: string;
  source_machine: string;
  target_machine: string;
  kind: EdgeKind;
  output_id: string | null;
  target_reference: string | null;
};
export type Scenario = {
  fixed_inputs: Record<string, TypedValue>;
  previous_states: Record<string, TypedValue>;
  previous_outputs: Record<string, TypedValue>;
};
export type ReviewRuleId =
  | 'upstream_changed_downstream_held'
  | 'previous_value_suppressed'
  | 'order_missed_update'
  | 'implicit_hold'
  | 'all_machines_held';
export type ReviewStatus = 'pending' | 'expected_by_design' | 'needs_correction' | 'deferred';
export type ReviewSettings = { enabled_rules: ReviewRuleId[]; enabled_machines: string[]; enabled_events: string[] };
export type ReviewDecision = {
  fingerprint: string;
  evidence_hash: string;
  status: ReviewStatus;
  note: string;
  updated_at: string;
  needs_recheck: boolean;
};
export type ImportSource = { file_name: string; sha256: string };
export type ProjectDto = {
  schema_version: number;
  project_id: string;
  name: string;
  revision: number;
  machines: Machine[];
  states: StateDefinition[];
  inputs: InputDefinition[];
  outputs: OutputDefinition[];
  events: EventDefinition[];
  transitions: TransitionRule[];
  graph_nodes: GraphNodeDto[];
  graph_edges: GraphEdgeDto[];
  scenario: Scenario;
  review_settings: ReviewSettings;
  review_decisions: ReviewDecision[];
  imports: ImportSource[];
};

export type Diagnostic = {
  code: string;
  severity: 'error' | 'warning' | 'info';
  message: string;
  detail: string | null;
  source: SourceLocation | null;
};
export type ProjectResponse = { project: ProjectDto; diagnostics: Diagnostic[] };
export type ImportResult = ProjectResponse;
export type MatrixMetadata = {
  revision: number;
  event_count: number;
  machine_count: number;
  combination_count: string;
  ordered_machines: string[];
};
export type CellStatus = 'changed' | 'explicit_hold' | 'implicit_hold' | 'indeterminate' | 'conflict';
export type MatrixCell = {
  event_id: string;
  combination_id: string;
  machine_id: string;
  current_state: string;
  next_state: string | null;
  status: CellStatus;
};
export type MatrixWindow = {
  revision: number;
  event_start: number;
  combination_start: string;
  event_count: number;
  combination_count: number;
  cells: MatrixCell[];
};
export type TruthValue = 'true' | 'false' | 'unknown';
export type ExpressionTrace = { expression: string; value: string; truth: TruthValue };
export type RuleEvaluation = {
  rule_id: string;
  priority: number;
  guard: TruthValue;
  destination: string;
  expressions: ExpressionTrace[];
};
export type MachineEvaluation = {
  machine_id: string;
  previous_state: string;
  next_state: string | null;
  status: CellStatus;
  selected_rule_id: string | null;
  evaluations: RuleEvaluation[];
  reason: string;
};
export type StepResult = {
  event_id: string;
  initial_states: Record<string, string>;
  next_states: Record<string, string | null>;
  evaluations: MachineEvaluation[];
};
export type ReviewCandidate = {
  fingerprint: string;
  evidence_hash: string;
  rule_id: ReviewRuleId;
  rule_code: string;
  event_id: string;
  event_name: string;
  combination_id: string;
  initial_states: Record<string, string>;
  machine_id: string;
  current_state: string | null;
  next_state: string | null;
  source_rule_id: string | null;
  reason: string;
  decision: ReviewDecision | null;
};
export type ReviewPage = { revision: number; offset: number; limit: number; total: number; candidates: ReviewCandidate[] };
export type ReviewCellKey = { event_id: string; combination_id: string; machine_id: string };
export type ReviewFilters = {
  event_id: string | null;
  event_ids?: string[] | null;
  machine_id: string | null;
  rule_code: string | null;
  decision: ReviewStatus | null;
  query: string | null;
  event_start?: number | null;
  event_count?: number | null;
  combination_start?: string | null;
  combination_count?: number | null;
};
export type ApiError = { code: string; message: string; detail: string | null; sourceLocation?: SourceLocation | null };

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const isDesktopApp = () => Boolean(window.__TAURI_INTERNALS__);

export async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktopApp()) {
    throw new Error('Excelの読込と状態遷移の解析にはSTATEFLOWデスクトップ版が必要です。');
  }
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new StateflowCommandError(error);
  }
}

export function getMatrixMetadata() {
  return invokeCommand<MatrixMetadata>('get_matrix_metadata');
}

export class StateflowCommandError extends Error {
  readonly code: string;
  readonly detail: string | null;
  readonly sourceLocation: SourceLocation | null;

  constructor(error: unknown) {
    const apiError = error && typeof error === 'object' ? error as Partial<ApiError> : null;
    super(apiError?.message ?? String(error));
    this.name = 'StateflowCommandError';
    this.code = apiError?.code ?? 'UNKNOWN_ERROR';
    this.detail = apiError?.detail ?? null;
    this.sourceLocation = apiError?.sourceLocation ?? null;
  }
}

export function typedValueFromText(value: string, type: ValueType): TypedValue {
  if (value === 'UNKNOWN') return { type: 'unknown' };
  switch (type) {
    case 'boolean':
      return { type: 'boolean', value: value === 'ON' || value === 'TRUE' };
    case 'enum':
      return { type: 'enum', value };
    case 'number':
      return { type: 'number', value: Number(value) };
    case 'string':
      return { type: 'string', value };
  }
}

export function valueText(value: TypedValue | null | undefined): string {
  if (!value || value.type === 'unknown') return 'UNKNOWN';
  if (value.type === 'boolean') return value.value ? 'ON' : 'OFF';
  return String(value.value);
}

export const reviewStatusLabels: Record<ReviewStatus, string> = {
  pending: '未判定',
  expected_by_design: '仕様通り',
  needs_correction: '要修正',
  deferred: '保留',
};

export const reviewRuleLabels: Record<ReviewRuleId, { code: string; name: string }> = {
  upstream_changed_downstream_held: { code: 'R01', name: '上流状態が変化したが参照する下流が状態維持' },
  previous_value_suppressed: { code: 'R02', name: '前回状態/前回出力条件が連動を抑止' },
  order_missed_update: { code: 'R03', name: '実行順序の関係で参照元の更新が評価時に未反映' },
  implicit_hold: { code: 'R04', name: '暗黙状態維持を抽出' },
  all_machines_held: { code: 'R05', name: '入力を変化させても全機能が状態維持' },
};
