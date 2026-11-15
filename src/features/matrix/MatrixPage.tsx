import { useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Chip, MetricCard, PageHeading, PanelHeader } from '../../components/ui';
import { invokeCommand, typedValueFromText, valueText, type CellStatus, type InputDefinition, type MatrixCell, type MatrixMetadata, type MatrixWindow, type ProjectDto, type ProjectResponse, type ReviewCellKey, type ReviewFilters, type ReviewPage, type Scenario, type StepResult } from '../../lib/tauri';
import { useUiStore } from '../../app/uiStore';

const EVENT_COLUMN_WIDTH = 174;
const STATE_COLUMN_WIDTH = 106;
const MAX_WINDOW_COMBINATIONS = 100_000n;

type SelectedCell = { eventId: string; combinationIndex: string; machineId: string };

export function MatrixPage(props: { onOpenReviews: () => void }) {
  const project = useUiStore((state) => state.project);
  const diagnostics = useUiStore((state) => state.diagnostics);
  const savedPath = useUiStore((state) => state.savedPath);
  const setProject = useUiStore((state) => state.setProject);
  const setDirty = useUiStore((state) => state.setDirty);
  const matrixFocus = useUiStore((state) => state.matrixFocus);
  const setMatrixFocus = useUiStore((state) => state.setMatrixFocus);
  const setReviewCandidateCount = useUiStore((state) => state.setReviewCandidateCount);
  const [metadata, setMetadata] = useState<MatrixMetadata | null>(null);
  const [matrixWindow, setMatrixWindow] = useState<MatrixWindow | null>(null);
  const [reviewPage, setReviewPage] = useState<ReviewPage | null>(null);
  const [reviewWindowKeys, setReviewWindowKeys] = useState<ReviewCellKey[]>([]);
  const [reviewError, setReviewError] = useState('');
  const [requestError, setRequestError] = useState('');
  const [trace, setTrace] = useState<StepResult | null>(null);
  const [traceError, setTraceError] = useState('');
  const [viewMode, setViewMode] = useState<'all' | 'review'>('all');
  const [machineFilter, setMachineFilter] = useState('all');
  const [windowStart, setWindowStart] = useState(0n);
  const [windowStartInput, setWindowStartInput] = useState('1');
  const [selected, setSelected] = useState<SelectedCell | null>(null);
  const [scenarioSaving, setScenarioSaving] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const matrixRequest = useRef(0);
  const traceRequest = useRef(0);

  const events = useMemo(() => project?.events.filter((event) => event.enabled) ?? [], [project]);
  const orderedMachines = metadata?.ordered_machines ?? [];
  const visibleMachines = useMemo(
    () => orderedMachines.filter((machineId) => machineFilter === 'all' || machineId === machineFilter),
    [machineFilter, orderedMachines],
  );
  const stateIdsByMachine = useMemo(() => {
    const result = new Map<string, string[]>();
    for (const state of project?.states ?? []) {
      const stateIds = result.get(state.machine_id) ?? [];
      stateIds.push(state.id);
      result.set(state.machine_id, stateIds);
    }
    return result;
  }, [project]);
  const combinationTotal = useMemo(() => BigInt(metadata?.combination_count ?? '0'), [metadata?.combination_count]);
  const remainingCombinations = combinationTotal > windowStart ? combinationTotal - windowStart : 0n;
  const combinationWindowCount = Number(remainingCombinations > MAX_WINDOW_COMBINATIONS ? MAX_WINDOW_COMBINATIONS : remainingCombinations);
  const visibleColumnCount = combinationWindowCount * visibleMachines.length;
  const virtualColumns = useVirtualizer({
    count: visibleColumnCount,
    getScrollElement: () => scrollRef.current,
    horizontal: true,
    estimateSize: () => STATE_COLUMN_WIDTH,
    overscan: 6,
  });
  const virtualRows = useVirtualizer({
    count: events.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 66,
    overscan: 5,
    getItemKey: (index) => events[index]?.id ?? index,
  });
  const visibleColumns = virtualColumns.getVirtualItems();
  const visibleRows = virtualRows.getVirtualItems();
  const firstColumn = visibleColumns[0];
  const lastColumn = visibleColumns[visibleColumns.length - 1];
  const firstRow = visibleRows[0];
  const lastRow = visibleRows[visibleRows.length - 1];
  const absoluteFirstCombination = windowStart + BigInt(Math.floor((firstColumn?.index ?? 0) / Math.max(1, visibleMachines.length)));
  const absoluteLastCombination = windowStart + BigInt(Math.floor((lastColumn?.index ?? 0) / Math.max(1, visibleMachines.length)));
  const matrixCells = useMemo(() => {
    const cells = new Map<string, MatrixCell>();
    for (const cell of matrixWindow?.cells ?? []) {
      cells.set(cellKey(cell.event_id, cell.combination_id, cell.machine_id), cell);
    }
    return cells;
  }, [matrixWindow]);
  const reviewCellKeys = useMemo(() => new Set(reviewWindowKeys.map((key) => {
    return cellKey(key.event_id, key.combination_id, key.machine_id);
  })), [reviewWindowKeys]);

  useEffect(() => {
    if (!project) {
      setMetadata(null);
      setMatrixWindow(null);
      setReviewPage(null);
      setReviewWindowKeys([]);
      return;
    }
    let active = true;
    setRequestError('');
    invokeCommand<MatrixMetadata>('get_matrix_metadata').then((result) => {
      if (active) {
        setMetadata(result);
        setWindowStart(0n);
        setWindowStartInput('1');
      }
    }).catch((error: unknown) => {
      if (active) setRequestError(errorMessage(error));
    });
    const filters: ReviewFilters = { event_id: null, machine_id: null, rule_code: null, decision: null, query: null };
    invokeCommand<ReviewPage>('get_review_page', { filters, offset: 0, limit: 200, revision: project.revision }).then((result) => {
      if (active) {
        setReviewPage(result);
        setReviewCandidateCount(result.total);
        setReviewError('');
      }
    }).catch((error: unknown) => {
      if (active) {
        setReviewPage(null);
        setReviewError(errorMessage(error));
      }
    });
    return () => { active = false; };
  }, [project]);

  useEffect(() => {
    if (!project || !metadata || !firstRow || !lastRow || !firstColumn || !lastColumn || visibleMachines.length === 0) {
      setMatrixWindow(null);
      return;
    }
    const requestNumber = ++matrixRequest.current;
    const eventStart = firstRow.index;
    const eventCount = lastRow.index - firstRow.index + 1;
    const combinationCount = Number(absoluteLastCombination - absoluteFirstCombination + 1n);
    setRequestError('');
    invokeCommand<MatrixWindow>('get_matrix_window', {
      eventStart,
      eventCount,
      combinationStart: absoluteFirstCombination.toString(),
      combinationCount,
      revision: metadata.revision,
    }).then((result) => {
      if (requestNumber === matrixRequest.current) setMatrixWindow(result);
    }).catch((error: unknown) => {
      if (requestNumber === matrixRequest.current) setRequestError(errorMessage(error));
    });
    invokeCommand<ReviewCellKey[]>('get_review_window', {
      eventIds: events.slice(eventStart, eventStart + eventCount).map((event) => event.id),
      combinationStart: absoluteFirstCombination.toString(),
      combinationCount,
      revision: metadata.revision,
    }).then((result) => {
      if (requestNumber === matrixRequest.current) setReviewWindowKeys(result);
    }).catch((error: unknown) => {
      if (requestNumber === matrixRequest.current) setReviewError(errorMessage(error));
    });
  }, [absoluteFirstCombination, absoluteLastCombination, events, firstColumn, firstRow, lastColumn, lastRow, metadata, project, visibleMachines.length]);

  useEffect(() => {
    if (!project || !selected || !metadata) {
      setTrace(null);
      return;
    }
    const requestNumber = ++traceRequest.current;
    setTraceError('');
    invokeCommand<StepResult>('get_cell_trace', {
      eventId: selected.eventId,
      combinationIndex: selected.combinationIndex,
      revision: metadata.revision,
    }).then((result) => {
      if (requestNumber === traceRequest.current) setTrace(result);
    }).catch((error: unknown) => {
      if (requestNumber === traceRequest.current) setTraceError(errorMessage(error));
    });
  }, [metadata, project, selected]);

  useEffect(() => {
    if (!project || !metadata || !matrixFocus) return;
    setSelected({ eventId: matrixFocus.eventId, combinationIndex: matrixFocus.combinationIndex, machineId: matrixFocus.machineId });
    jumpToCombination(BigInt(matrixFocus.combinationIndex));
    setMatrixFocus(null);
  }, [matrixFocus, metadata, project, setMatrixFocus]);

  useEffect(() => {
    if (!project || !events.length || !orderedMachines.length || matrixFocus) return;
    if (!selected || !events.some((event) => event.id === selected.eventId) || !orderedMachines.includes(selected.machineId)) {
      setSelected({ eventId: events[0].id, combinationIndex: '0', machineId: orderedMachines[0] });
    }
  }, [events, matrixFocus, orderedMachines, project, selected]);

  const selectedEvent = events.find((event) => event.id === selected?.eventId) ?? null;
  const selectedComboIndex = selected ? BigInt(selected.combinationIndex) : 0n;
  const selectedCombinationId = combinationId(selectedComboIndex);
  const selectedStates = project && orderedMachines.length > 0 ? combinationAt(project, orderedMachines, selectedComboIndex) : {};
  const selectedMachine = project?.machines.find((machine) => machine.id === selected?.machineId) ?? null;
  const selectedEvaluation = trace?.evaluations.find((evaluation) => evaluation.machine_id === selected?.machineId) ?? null;
  const checkedCellCount = BigInt(events.length) * combinationTotal * BigInt(orderedMachines.length);
  const reviewCount = reviewPage?.total;
  const fixedInputs = project?.inputs.filter((input) => !events.some((event) => event.input_id === input.id)) ?? [];

  function jumpToCombination(index: bigint) {
    if (combinationTotal === 0n) return;
    const lastIndex = combinationTotal - 1n;
    const nextStart = index > lastIndex ? lastIndex : index;
    setWindowStart(nextStart);
    setWindowStartInput((nextStart + 1n).toString());
    requestAnimationFrame(() => {
      if (scrollRef.current) scrollRef.current.scrollLeft = 0;
    });
  }

  function commitWindowStart() {
    try {
      const nextStart = BigInt(windowStartInput.trim() || '1') - 1n;
      jumpToCombination(nextStart < 0n ? 0n : nextStart);
    } catch {
      setWindowStartInput((windowStart + 1n).toString());
    }
  }

  async function updateScenario(change: Partial<Scenario>) {
    if (!project) return;
    const scenario: Scenario = {
      fixed_inputs: { ...project.scenario.fixed_inputs, ...change.fixed_inputs },
      previous_states: { ...project.scenario.previous_states, ...change.previous_states },
      previous_outputs: { ...project.scenario.previous_outputs, ...change.previous_outputs },
    };
    setScenarioSaving(true);
    try {
      const result = await invokeCommand<ProjectResponse>('update_scenario', { scenario, baseRevision: project.revision });
      setProject(result.project, result.diagnostics, savedPath);
      setDirty(true);
    } catch (error) {
      setRequestError(errorMessage(error));
    } finally {
      setScenarioSaving(false);
    }
  }

  function setFixedInput(input: InputDefinition, value: string) {
    void updateScenario({ fixed_inputs: { [input.id]: typedValueFromText(value, input.value_type) } });
  }

  if (!project) {
    return <section className="page-section">
      <PageHeading eyebrow="MATRIX INSPECTION" title="" subtitle="縦軸：イベント変化パターン　/　横軸：状態組み合わせ内の各機能の現在状態　/　セル：遷移後の状態" />
      <div className="card empty-project"><strong>解析するSTATEFLOWプロジェクトがありません。</strong><p>上部の「Excel取込」から、Machines / States / Inputs / Outputs / Events / Transitionsのブックを選択してください。</p></div>
    </section>;
  }

  const columnStartSize = firstColumn?.start ?? 0;
  const columnEndSize = lastColumn ? Math.max(0, virtualColumns.getTotalSize() - lastColumn.start - lastColumn.size) : 0;
  const rowStartSize = firstRow?.start ?? 0;
  const rowEndSize = lastRow ? Math.max(0, virtualRows.getTotalSize() - lastRow.end) : 0;
  const totalColumns = 2 + visibleColumnCount;

  return (
    <section className="page-section">
      <PageHeading
        eyebrow="MATRIX INSPECTION"
        title=""
        subtitle="縦軸：イベント変化パターン　/　横軸：状態組み合わせ内の各機能の現在状態　/　セル：遷移後の状態"
      />

      <div className="card matrix-controls">
        {fixedInputs.map((input) => <label className="field" key={input.id}>
          <span>{input.name} / {input.id}</span>
          {input.value_type === 'boolean' ? <select value={valueText(project.scenario.fixed_inputs[input.id] ?? input.default_value)} onChange={(event) => setFixedInput(input, event.target.value)}>
            <option value="OFF">OFF</option><option value="ON">ON</option><option value="UNKNOWN">UNKNOWN</option>
          </select> : input.value_type === 'enum' ? <select value={valueText(project.scenario.fixed_inputs[input.id] ?? input.default_value)} onChange={(event) => setFixedInput(input, event.target.value)}>
            {(input.domain.kind === 'enum' ? input.domain.values : []).map((value) => <option key={value}>{value}</option>)}
            <option value="UNKNOWN">UNKNOWN</option>
          </select> : <input type={input.value_type === 'number' ? 'number' : 'text'} value={valueText(project.scenario.fixed_inputs[input.id] ?? input.default_value)} onBlur={(event) => setFixedInput(input, event.currentTarget.value)} />}
        </label>)}
        {project.outputs.map((output) => <label className="field" key={`output-${output.id}`}>
          <span>前回出力：{output.name}</span>
          {output.value_type === 'boolean' ? <select value={valueText(project.scenario.previous_outputs[output.id] ?? output.default_previous_value)} onChange={(event) => void updateScenario({ previous_outputs: { [output.id]: typedValueFromText(event.target.value, output.value_type) } })}>
            <option value="OFF">OFF</option><option value="ON">ON</option><option value="UNKNOWN">UNKNOWN</option>
          </select> : output.value_type === 'enum' ? <select value={valueText(project.scenario.previous_outputs[output.id] ?? output.default_previous_value)} onChange={(event) => void updateScenario({ previous_outputs: { [output.id]: typedValueFromText(event.target.value, output.value_type) } })}>
            {(output.domain.kind === 'enum' ? output.domain.values : []).map((value) => <option key={value}>{value}</option>)}<option value="UNKNOWN">UNKNOWN</option>
          </select> : <input type={output.value_type === 'number' ? 'number' : 'text'} value={valueText(project.scenario.previous_outputs[output.id] ?? output.default_previous_value)} onBlur={(event) => void updateScenario({ previous_outputs: { [output.id]: typedValueFromText(event.currentTarget.value, output.value_type) } })} />}
        </label>)}
        <details className="history-controls">
          <summary>前回状態を設定</summary>
          <div>{project.machines.map((machine) => <label className="field" key={`previous-${machine.id}`}>
            <span>PREV.STATE.{machine.id}</span>
            <select value={valueText(project.scenario.previous_states[machine.id])} onChange={(event) => void updateScenario({ previous_states: { [machine.id]: typedValueFromText(event.target.value, 'enum') } })}>
              <option value="UNKNOWN">UNKNOWN</option>
              {(stateIdsByMachine.get(machine.id) ?? []).map((stateId) => <option key={stateId}>{stateId}</option>)}
            </select>
          </label>)}</div>
        </details>
        <label className="field"><span>表示モード</span><select value={viewMode} onChange={(event) => setViewMode(event.target.value as 'all' | 'review')}><option value="all">全セル</option><option value="review">レビュー候補を強調</option></select></label>
        <label className="field"><span>機能列を表示</span><select value={machineFilter} onChange={(event) => setMachineFilter(event.target.value)}><option value="all">すべての機能</option>{orderedMachines.map((machineId) => <option key={machineId} value={machineId}>{machineName(project, machineId)}</option>)}</select></label>
        <label className="field combination-jump"><span>組み合わせへ移動</span><input aria-label="組み合わせ番号" inputMode="numeric" value={windowStartInput} onChange={(event) => setWindowStartInput(event.target.value)} onBlur={commitWindowStart} onKeyDown={(event) => { if (event.key === 'Enter') commitWindowStart(); }} /><small>1〜{combinationTotal.toString()}</small></label>
        <p className="control-explain">固定入力は各行で共通。イベントのみ1項目変化し、各機能を設定順に1回ずつ評価します。{scenarioSaving ? ' 保存中…' : ''}</p>
      </div>

      <div className="metrics-grid">
        <MetricCard label="イベント変化" value={events.length.toLocaleString()} note="入力の立上り・立下り" />
        <MetricCard label="状態組み合わせ" value={combinationTotal.toLocaleString()} note={orderedMachines.map((id) => stateIdsByMachine.get(id)?.length ?? 0).join(' × ')} />
        <MetricCard label="検証セル" value={checkedCellCount.toLocaleString()} note={`${events.length}行 × ${combinationTotal.toLocaleString()}組 × ${orderedMachines.length}機能`} />
        <MetricCard alert label="連動レビュー候補" value={reviewCount?.toLocaleString() ?? '—'} note={reviewError ? '抽出範囲の上限または検証エラー' : '状態維持＝異常とは限らない'} />
      </div>

      {requestError ? <div className="diagnostics-banner" role="alert"><strong>マトリクスを取得できません</strong><span>{requestError}</span></div> : null}
      {diagnostics.some((diagnostic) => diagnostic.severity === 'error') ? <div className="small-summary"><strong>解析を開始できません。</strong> エラーの内容を確認し、依存関係画面で不足する接続を追加してください。</div> : null}
      {reviewError ? <div className="small-summary"><strong>レビュー候補：</strong> {reviewError}</div> : null}

      <div className="matrix-layout">
        <section className="card matrix-card" aria-label="イベントと現在状態のマトリクス">
          <PanelHeader title="イベント × 現在状態マトリクス" trailing={
            <div className="matrix-panel-tools">
              <button className="button small" onClick={props.onOpenReviews} type="button">⚑ レビュー候補一覧</button>
              <span className="legend-item"><i className="legend-swatch changed" />状態変化</span>
              <span className="legend-item"><i className="legend-swatch hold" />状態維持</span>
              <span className="legend-item"><i className="legend-swatch warn" />連動要確認</span>
            </div>
          } />
          <div className="matrix-scroll" ref={scrollRef}>
            <table className="matrix-table virtual-matrix" style={{ width: EVENT_COLUMN_WIDTH + virtualColumns.getTotalSize() }}>
              <thead>
                <tr>
                  <th className="sticky-event axis" rowSpan={2} style={{ width: EVENT_COLUMN_WIDTH, minWidth: EVENT_COLUMN_WIDTH }}>イベント変化 ↓</th>
                  {firstColumn ? <th className="virtual-spacer" aria-hidden="true" style={{ width: columnStartSize, minWidth: columnStartSize }} /> : null}
                  {visibleGroupColumns(visibleColumns, visibleMachines.length).map((group) => (
                    <th className="group-heading" colSpan={group.count} key={group.combinationIndex} style={{ width: group.count * STATE_COLUMN_WIDTH, minWidth: group.count * STATE_COLUMN_WIDTH }}>
                      <button className="combination-link" type="button" onClick={() => jumpToCombination(windowStart + BigInt(group.combinationIndex))}>
                        組み合わせ{String(Number(windowStart + BigInt(group.combinationIndex)) + 1).padStart(2, '0')}
                      </button>
                    </th>
                  ))}
                  {lastColumn ? <th className="virtual-spacer" aria-hidden="true" style={{ width: columnEndSize, minWidth: columnEndSize }} /> : null}
                </tr>
                <tr>
                  {firstColumn ? <th className="virtual-spacer" aria-hidden="true" style={{ width: columnStartSize, minWidth: columnStartSize }} /> : null}
                  {visibleColumns.map((column) => {
                    const machineId = visibleMachines[column.index % visibleMachines.length];
                    const combinationIndex = windowStart + BigInt(Math.floor(column.index / visibleMachines.length));
                    const states = project && orderedMachines.length > 0 ? combinationAt(project, orderedMachines, combinationIndex) : {};
                    return <th className={`machine-heading${column.index % visibleMachines.length === 0 ? ' group-start' : ''}`} key={column.key} style={{ width: column.size, minWidth: column.size }}>
                      <b>{machineShortName(project, machineId)}</b><small>現在：{states[machineId] ?? '—'}</small>
                    </th>;
                  })}
                  {lastColumn ? <th className="virtual-spacer" aria-hidden="true" style={{ width: columnEndSize, minWidth: columnEndSize }} /> : null}
                </tr>
              </thead>
              <tbody>
                {firstRow && firstRow.start > 0 ? <tr aria-hidden="true" className="virtual-spacer-row" style={{ height: rowStartSize }}><td colSpan={totalColumns} /></tr> : null}
                {visibleRows.map((row) => {
                  const event = events[row.index];
                  if (!event) return null;
                  return <tr key={event.id} aria-rowindex={row.index + 2} style={{ height: row.size }}>
                    <th className="event-heading sticky-event" scope="row" style={{ width: EVENT_COLUMN_WIDTH, minWidth: EVENT_COLUMN_WIDTH }}>
                      <strong>{event.name}</strong><span>{inputName(project, event.input_id)}　{valueText(event.from_value)} → {valueText(event.to_value)}</span>
                    </th>
                    {firstColumn ? <td className="virtual-spacer" aria-hidden="true" style={{ width: columnStartSize, minWidth: columnStartSize }} /> : null}
                    {visibleColumns.map((column) => {
                      const machineId = visibleMachines[column.index % visibleMachines.length];
                      const comboOffset = Math.floor(column.index / visibleMachines.length);
                      const comboIndex = windowStart + BigInt(comboOffset);
                      const comboId = combinationId(comboIndex);
                      const states = project && orderedMachines.length > 0 ? combinationAt(project, orderedMachines, comboIndex) : {};
                      const cell = matrixCells.get(cellKey(event.id, comboId, machineId));
                      const currentState = states[machineId] ?? '';
                      const nextState = cell?.next_state ?? (cell ? 'UNKNOWN' : '…');
                      const changed = cell ? cell.status === 'changed' : nextState !== currentState;
                      const reviewCandidate = reviewCellKeys.has(cellKey(event.id, comboId, machineId));
                      const isSelected = selected?.eventId === event.id && selected?.combinationIndex === comboIndex.toString() && selected?.machineId === machineId;
                      return <td className={`next-state ${changed ? 'changed' : 'held'}${column.index % visibleMachines.length === 0 ? ' group-start' : ''}${reviewCandidate ? ' review-candidate' : ''}${viewMode === 'review' && !reviewCandidate ? ' review-muted' : ''}${isSelected ? ' selected' : ''}`}
                        key={`${event.id}-${comboId}-${machineId}`} style={{ width: column.size, minWidth: column.size }}>
                        <button aria-label={`${event.name}, ${comboId}, ${machineName(project, machineId)}: 次状態 ${nextState}`}
                          disabled={!cell}
                          onClick={() => setSelected({ eventId: event.id, combinationIndex: comboIndex.toString(), machineId })} type="button">
                          {nextState}
                        </button>
                      </td>;
                    })}
                    {lastColumn ? <td className="virtual-spacer" aria-hidden="true" style={{ width: columnEndSize, minWidth: columnEndSize }} /> : null}
                  </tr>;
                })}
                {lastRow && lastRow.end < virtualRows.getTotalSize() ? <tr aria-hidden="true" className="virtual-spacer-row" style={{ height: rowEndSize }}><td colSpan={totalColumns} /></tr> : null}
              </tbody>
            </table>
          </div>
          <div className="table-foot">横方向にスクロールして各状態組み合わせを比較できます。必要なイベント行と状態組み合わせだけを取得し、セルには次状態のみを表示します。</div>
        </section>

        <aside className="card detail-panel" aria-label="選択セルの評価詳細">
          <div className="eyebrow">CELL INSPECTOR / {selectedCombinationId}</div>
          <div className="detail-title">{selectedMachine?.name ?? selected?.machineId ?? '機能'}：{selected ? selectedStates[selected.machineId] ?? 'UNKNOWN' : '—'} → {selectedEvaluation?.next_state ?? 'UNKNOWN'}</div>
          <div className="detail-pair"><span>入力イベント</span><strong>{selectedEvent?.name ?? '—'}<br />{selectedEvent ? `${valueText(selectedEvent.from_value)} → ${valueText(selectedEvent.to_value)}` : ''}</strong></div>
          <div className="detail-pair"><span>評価前の状態</span><strong>{orderedMachines.map((machineId) => `${machineShortName(project, machineId)} ${selectedStates[machineId] ?? '—'}`).join(' / ')}</strong></div>
          <div className="detail-pair"><span>固定入力</span><strong>{fixedInputs.map((input) => `${input.id} ${valueText(project.scenario.fixed_inputs[input.id] ?? input.default_value)}`).join(' / ') || 'なし'}</strong></div>
          <div className="detail-pair"><span>前回出力</span><strong>{project.outputs.map((output) => `${output.id} ${valueText(project.scenario.previous_outputs[output.id] ?? output.default_previous_value)}`).join(' / ') || 'なし'}</strong></div>
          <h3 className="detail-section-title">逐次評価トレース</h3>
          {trace?.evaluations.map((evaluation) => <div className={`trace-step trace-${evaluation.status}`} key={evaluation.machine_id}>
            <div><strong>{evaluation.machine_id}</strong><span>{evaluation.previous_state} → {evaluation.next_state ?? '判定不能'}</span></div>
            <small>{evaluation.reason}</small>
            {evaluation.evaluations.map((rule) => <div className="rule-trace" key={rule.rule_id}>
              <strong>{rule.rule_id} / priority {rule.priority} / {truthLabel(rule.guard)}</strong>
              {rule.expressions.map((expression, index) => <small className={`expression-truth truth-${expression.truth}`} key={`${rule.rule_id}-${index}`}>{expression.expression}　{expression.value} ({truthLabel(expression.truth)})</small>)}
            </div>)}
          </div>)}
          {traceError ? <p className="inline-error">{traceError}</p> : null}
          {selectedEvaluation ? <div className="detail-pair"><span>判定</span><strong>{statusLabel(selectedEvaluation.status)}</strong></div> : null}
          {selected ? <div className="review-note"><strong>レビュー候補</strong><p>{reviewCellKeys.has(cellKey(selected.eventId, selectedCombinationId, selected.machineId)) ? 'このセルは現在の抽出設定に一致しています。候補一覧で理由と判定を確認してください。' : 'このセルは現在の抽出設定ではレビュー候補に含まれていません。'}</p></div> : null}
          {selectedRuleSource(project, selectedEvaluation?.selected_rule_id) ? <Chip>出典：{selectedRuleSource(project, selectedEvaluation?.selected_rule_id)}</Chip> : null}
        </aside>
      </div>
      <p className="footer-hint">全機能1ステップ検証です。イベント行ごとに同じ開始条件から独立して評価します。UNKNOWNや競合は確定した次状態として表示しません。</p>
    </section>
  );
}

function visibleGroupColumns(items: { index: number }[], machineCount: number) {
  const groups: { combinationIndex: number; count: number }[] = [];
  for (const item of items) {
    const combinationIndex = Math.floor(item.index / Math.max(1, machineCount));
    const last = groups[groups.length - 1];
    if (last?.combinationIndex === combinationIndex) last.count += 1;
    else groups.push({ combinationIndex, count: 1 });
  }
  return groups;
}

function combinationAt(project: ProjectDto, orderedMachines: string[], index: bigint): Record<string, string> {
  const result: Record<string, string> = {};
  let remaining = index;
  for (const machineId of [...orderedMachines].reverse()) {
    const states = project.states.filter((state) => state.machine_id === machineId);
    if (states.length === 0) continue;
    const stateIndex = Number(remaining % BigInt(states.length));
    remaining /= BigInt(states.length);
    result[machineId] = states[stateIndex].id;
  }
  return result;
}

function combinationId(index: bigint) {
  return `COMBO-${(index + 1n).toString().padStart(12, '0')}`;
}

function cellKey(eventId: string, combination: string, machineId: string) {
  return `${eventId}\u0000${combination}\u0000${machineId}`;
}

function machineName(project: ProjectDto | null, machineId: string) {
  return project?.machines.find((machine) => machine.id === machineId)?.name ?? machineId;
}

function machineShortName(project: ProjectDto | null, machineId: string) {
  return project?.machines.find((machine) => machine.id === machineId)?.id ?? machineId;
}

function inputName(project: ProjectDto, inputId: string) {
  return project.inputs.find((input) => input.id === inputId)?.name ?? inputId;
}

function statusLabel(status: CellStatus) {
  const labels: Record<CellStatus, string> = {
    changed: '状態変化',
    explicit_hold: '明示状態維持',
    implicit_hold: '暗黙状態維持',
    indeterminate: '判定不能',
    conflict: '同順位競合',
  };
  return labels[status];
}

function truthLabel(truth: 'true' | 'false' | 'unknown') {
  return truth === 'true' ? 'TRUE' : truth === 'false' ? 'FALSE' : 'UNKNOWN';
}

function selectedRuleSource(project: ProjectDto, selectedRuleId: string | null | undefined) {
  if (!selectedRuleId) return null;
  const rule = project.transitions.find((candidate) => candidate.id === selectedRuleId);
  if (!rule) return null;
  return `${rule.id}${rule.source ? ` / ${rule.source.file_name}:${rule.source.sheet}!${rule.source.column}${rule.source.row}` : ''}`;
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
