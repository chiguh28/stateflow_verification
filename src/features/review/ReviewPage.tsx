import { type FormEvent, useEffect, useMemo, useState } from 'react';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { Chip, MetricCard, PageHeading, PanelHeader } from '../../components/ui';
import { useUiStore } from '../../app/uiStore';
import { invokeCommand, reviewRuleLabels, reviewStatusLabels, type ProjectResponse, type ReviewCandidate, type ReviewFilters, type ReviewPage as ReviewPageResult, type ReviewStatus, type StepResult } from '../../lib/tauri';

const PAGE_SIZE = 50;
const statuses: ReviewStatus[] = ['pending', 'expected_by_design', 'needs_correction', 'deferred'];

export function ReviewPage(props: { onShowMatrix: () => void; onOpenSettings: () => void }) {
  const project = useUiStore((state) => state.project);
  const savedPath = useUiStore((state) => state.savedPath);
  const setProject = useUiStore((state) => state.setProject);
  const setDirty = useUiStore((state) => state.setDirty);
  const setReviewCandidateCount = useUiStore((state) => state.setReviewCandidateCount);
  const [query, setQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<ReviewStatus | 'all'>('all');
  const [ruleFilter, setRuleFilter] = useState('all');
  const [page, setPage] = useState<ReviewPageResult | null>(null);
  const [offset, setOffset] = useState(0);
  const [selectedFingerprint, setSelectedFingerprint] = useState('');
  const [draftNote, setDraftNote] = useState('');
  const [trace, setTrace] = useState<StepResult | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  const filters: ReviewFilters = {
    event_id: null,
    machine_id: null,
    rule_code: ruleFilter === 'all' ? null : ruleFilter,
    decision: statusFilter === 'all' ? null : statusFilter,
    query: query.trim() || null,
  };

  useEffect(() => {
    if (!project) {
      setPage(null);
      setTrace(null);
      setError('');
      return;
    }
    let active = true;
    setError('');
    invokeCommand<ReviewPageResult>('get_review_page', { filters, offset, limit: PAGE_SIZE, revision: project.revision })
      .then((result) => {
        if (!active) return;
        setPage(result);
        setReviewCandidateCount(result.total);
        setSelectedFingerprint((current) => result.candidates.some((candidate) => candidate.fingerprint === current)
          ? current
          : result.candidates[0]?.fingerprint ?? '');
      })
      .catch((reason: unknown) => { if (active) setError(errorMessage(reason)); });
    return () => { active = false; };
  }, [project, offset, query, ruleFilter, statusFilter]);

  const candidates = page?.candidates ?? [];
  const selected = candidates.find((candidate) => candidate.fingerprint === selectedFingerprint) ?? null;
  const displayedPending = candidates.filter((candidate) => !candidate.decision || candidate.decision.needs_recheck).length;
  const displayedAccepted = candidates.filter((candidate) => candidate.decision?.status === 'expected_by_design' && !candidate.decision.needs_recheck).length;
  const displayedCorrection = candidates.filter((candidate) => candidate.decision?.status === 'needs_correction' && !candidate.decision.needs_recheck).length;

  useEffect(() => {
    if (!project || !selected) {
      setTrace(null);
      return;
    }
    const combinationIndex = combinationIndexFromId(selected.combination_id);
    let active = true;
    invokeCommand<StepResult>('get_cell_trace', { eventId: selected.event_id, combinationIndex: combinationIndex.toString(), revision: project.revision })
      .then((result) => { if (active) setTrace(result); })
      .catch((reason: unknown) => { if (active) setError(errorMessage(reason)); });
    return () => { active = false; };
  }, [project, selected]);

  useEffect(() => {
    setDraftNote(selected?.decision?.note ?? '');
  }, [selected?.fingerprint, selected?.decision?.note]);

  const eventOptions = useMemo(() => project?.events ?? [], [project]);

  async function saveDecision(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!project || !selected) return;
    const formData = new FormData(event.currentTarget);
    const status = String(formData.get('review-status')) as ReviewStatus;
    setBusy(true);
    setError('');
    try {
      const result = await invokeCommand<ProjectResponse>('save_review_decision', {
        fingerprint: selected.fingerprint,
        evidenceHash: selected.evidence_hash,
        status,
        note: draftNote,
        baseRevision: project.revision,
      });
      setProject(result.project, result.diagnostics, savedPath);
      setDirty(true);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function exportCsv() {
    if (!project) return;
    const path = await saveDialog({ defaultPath: `${project.name}-reviews.csv`, filters: [{ name: 'CSV', extensions: ['csv'] }] });
    if (!path) return;
    setBusy(true);
    try {
      const count = await invokeCommand<number>('export_reviews_csv', { path, filters, revision: project.revision });
      setError('');
      window.alert(`${count.toLocaleString()}件のレビュー候補をCSVに出力しました。`);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  function showInMatrix() {
    if (selected) {
      useUiStore.getState().setMatrixFocus({ eventId: selected.event_id, combinationIndex: combinationIndexFromId(selected.combination_id).toString(), machineId: selected.machine_id });
    }
    props.onShowMatrix();
  }

  if (!project) {
    return <section className="page-section"><PageHeading eyebrow="LINKAGE REVIEW / FINDINGS" title="連動レビュー候補一覧" subtitle="プロジェクトを開くと候補を抽出します。" /><div className="card empty-project">レビュー対象のSTATEFLOWプロジェクトがありません。</div></section>;
  }

  const pageEnd = Math.min(offset + PAGE_SIZE, page?.total ?? 0);

  return (
    <section className="page-section">
      <PageHeading eyebrow="LINKAGE REVIEW / FINDINGS" title="連動レビュー候補一覧" subtitle="Rustコアが抽出した候補について、逐次評価の根拠を確認して設計者の判断を保存します。"
        actions={<><Chip>{project.review_settings.enabled_rules.length}件の検出ルール有効</Chip><button className="button" onClick={props.onOpenSettings} type="button">⚙ 抽出条件を設定</button><button className="button" disabled={busy} onClick={() => void exportCsv()} type="button">↓ 候補CSV</button></>} />
      {error ? <div className="diagnostics-banner" role="alert"><strong>レビューを取得できません</strong><span>{error}</span></div> : null}
      <div className="metrics-grid review-metrics">
        <MetricCard label="レビュー候補" value={page?.total.toLocaleString() ?? '—'} note="現在の抽出条件に一致" />
        <MetricCard alert label="表示中・未判定" value={displayedPending} note={`${candidates.length}件の表示範囲`} />
        <MetricCard label="表示中・仕様通り" value={displayedAccepted} note="現在のページ" />
        <MetricCard label="表示中・要修正" value={displayedCorrection} note="現在のページ" />
      </div>
      <div className="card review-filters">
        <label className="field search-field"><span>候補検索</span><input value={query} onChange={(event) => { setQuery(event.target.value); setOffset(0); }} placeholder="イベント・機能・条件を検索" /></label>
        <label className="field"><span>判定ステータス</span><select value={statusFilter} onChange={(event) => { setStatusFilter(event.target.value as ReviewStatus | 'all'); setOffset(0); }}><option value="all">すべて</option>{statuses.map((status) => <option key={status} value={status}>{reviewStatusLabels[status]}</option>)}</select></label>
        <label className="field"><span>検出理由</span><select value={ruleFilter} onChange={(event) => { setRuleFilter(event.target.value); setOffset(0); }}><option value="all">すべての理由</option>{Object.values(reviewRuleLabels).map((rule) => <option key={rule.code} value={rule.code}>{rule.code} / {rule.name}</option>)}</select></label>
        <button className="button small" onClick={() => { setQuery(''); setStatusFilter('all'); setRuleFilter('all'); setOffset(0); }} type="button">フィルタ解除</button>
      </div>

      <div className="review-layout">
        <section className="card review-table-card">
          <PanelHeader title="抽出結果" trailing={<Chip>{candidates.length} / {page?.total.toLocaleString() ?? '—'} 件</Chip>} />
          <div className="review-table-scroll">
            <table className="review-table">
              <thead><tr><th>候補ID</th><th>イベント変化</th><th>操作前の状態組み合わせ</th><th>対象機能</th><th>現在 → 次状態</th><th>抽出理由</th><th>レビュー判定</th></tr></thead>
              <tbody>
                {candidates.map((candidate) => {
                  const event = eventOptions.find((item) => item.id === candidate.event_id);
                  const machine = project.machines.find((item) => item.id === candidate.machine_id);
                  const status = candidate.decision?.needs_recheck ? 'pending' : candidate.decision?.status ?? 'pending';
                  return <tr className={candidate.fingerprint === selectedFingerprint ? 'selected' : ''} key={candidate.fingerprint} onClick={() => setSelectedFingerprint(candidate.fingerprint)}>
                    <td><strong>{candidate.rule_code}</strong><small>{candidate.fingerprint.slice(0, 12)}</small></td>
                    <td>{candidate.event_name}<small>{event ? `${valueText(event.from_value)} → ${valueText(event.to_value)}` : candidate.event_id}</small></td>
                    <td><small>{Object.entries(candidate.initial_states).map(([id, state]) => `${id}: ${state}`).join(' / ')}</small><small>{candidate.combination_id}</small></td>
                    <td>{machine?.name ?? candidate.machine_id}</td>
                    <td><strong>{candidate.current_state ?? 'UNKNOWN'} → {candidate.next_state ?? '判定不能'}</strong></td>
                    <td><span className="reason-tag">{reviewRuleLabels[candidate.rule_id].name}</span>{candidate.decision?.needs_recheck ? <small>根拠が更新されています。再確認してください。</small> : null}</td>
                    <td><span className={`review-status ${statusClass(status)}`}>{reviewStatusLabels[status]}</span></td>
                  </tr>;
                })}
                {candidates.length === 0 ? <tr><td className="empty-table" colSpan={7}>該当するレビュー候補はありません。検索条件や抽出ルールを調整してください。</td></tr> : null}
              </tbody>
            </table>
          </div>
          <div className="review-pagination"><span>{page?.total ? `${offset + 1}–${pageEnd} / ${page.total.toLocaleString()} 件` : '候補なし'}</span><div><button className="button small" disabled={offset === 0 || busy} onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))} type="button">← 前へ</button><button className="button small" disabled={!page || offset + PAGE_SIZE >= page.total || busy} onClick={() => setOffset(offset + PAGE_SIZE)} type="button">次へ →</button></div></div>
          <div className="table-foot">候補行を選択すると、入力条件・逐次評価トレース・レビュー判断を右側で確認できます。</div>
        </section>

        <aside className="card detail-panel review-inspector">
          {selected ? <ReviewInspector project={project} candidate={selected} trace={trace} draftNote={draftNote} onDraftNoteChange={setDraftNote} onSave={saveDecision} onShowMatrix={showInMatrix} busy={busy} /> : <p className="note">レビュー候補を選択すると詳細を表示します。</p>}
        </aside>
      </div>
      <p className="footer-hint">自動抽出は設計不具合の確定判定ではありません。仕様通り・要修正・保留は設計者が判断します。レビュー結果はプロジェクト保存で永続化されます。</p>
    </section>
  );
}

function ReviewInspector(props: {
  project: NonNullable<ReturnType<typeof useUiStore.getState>['project']>;
  candidate: ReviewCandidate;
  trace: StepResult | null;
  draftNote: string;
  onDraftNoteChange: (note: string) => void;
  onSave: (event: FormEvent<HTMLFormElement>) => void;
  onShowMatrix: () => void;
  busy: boolean;
}) {
  const { candidate, project } = props;
  const event = project.events.find((item) => item.id === candidate.event_id);
  const machine = project.machines.find((item) => item.id === candidate.machine_id);
  const sourceRule = candidate.source_rule_id ? project.transitions.find((item) => item.id === candidate.source_rule_id) : null;
  const combinationIndex = combinationIndexFromId(candidate.combination_id);
  const currentDecision = candidate.decision?.needs_recheck ? 'pending' : candidate.decision?.status ?? 'pending';

  return <>
    <div className="eyebrow">REVIEW INSPECTOR / {candidate.combination_id}</div>
    <div className="detail-title">{machine?.name ?? candidate.machine_id}：{candidate.current_state ?? 'UNKNOWN'} → {candidate.next_state ?? '判定不能'}</div>
    <div className="detail-pair"><span>入力イベント</span><strong>{event?.name ?? candidate.event_name}<br />{event ? `${valueText(event.from_value)} → ${valueText(event.to_value)}` : candidate.event_id}</strong></div>
    <div className="detail-pair"><span>現在の状態</span><strong>{Object.entries(candidate.initial_states).map(([id, state]) => `${id} ${state}`).join(' / ')}</strong></div>
    <div className="detail-pair"><span>出典ルール</span><strong>{sourceRule ? `${sourceRule.id} / priority ${sourceRule.priority}${sourceRule.source_ref ? ` / ${sourceRule.source_ref}` : ''}` : candidate.source_rule_id ?? '該当ルールなし'}</strong></div>
    <div className="review-reasons"><div className="review-reason"><strong>{reviewRuleLabels[candidate.rule_id].code} / {reviewRuleLabels[candidate.rule_id].name}</strong><p>{candidate.reason}</p></div></div>
    <h3 className="detail-section-title">逐次評価トレース</h3>
    {props.trace?.evaluations.map((step) => <div className={`trace-step trace-${step.status}`} key={step.machine_id}>
      <div><strong>{step.machine_id}</strong><span>{step.previous_state} → {step.next_state ?? '判定不能'}</span></div>
      <small>{step.reason}</small>
      {step.evaluations.map((rule) => <div className="rule-trace" key={rule.rule_id}><strong>{rule.rule_id} / priority {rule.priority} / {rule.guard.toUpperCase()}</strong>{rule.expressions.map((expression, index) => <small className={`expression-truth truth-${expression.truth}`} key={`${rule.rule_id}-${index}`}>{expression.expression}　{expression.value} ({expression.truth.toUpperCase()})</small>)}</div>)}
    </div>)}
    <form className="review-decision-form" onSubmit={props.onSave}>
      <label className="field"><span>設計者のレビュー判定</span><select defaultValue={currentDecision} key={`${candidate.fingerprint}-${currentDecision}`} name="review-status">{statuses.map((status) => <option key={status} value={status}>{reviewStatusLabels[status]}</option>)}</select></label>
      <label className="field"><span>判定根拠・対応メモ</span><textarea value={props.draftNote} onChange={(event) => props.onDraftNoteChange(event.target.value)} placeholder="設計判断の根拠や対応内容を記録" /></label>
      <div className="detail-actions"><button className="button primary" disabled={props.busy} type="submit">判定を保存</button><button className="button" onClick={props.onShowMatrix} type="button">マトリクスで確認 →</button></div>
    </form>
    <div className="detail-pair"><span>追跡ID</span><strong><code>{candidate.fingerprint.slice(0, 16)}</code></strong></div>
    <div className="detail-pair"><span>組み合わせ番号</span><strong>{(combinationIndex + 1n).toString()}</strong></div>
  </>;
}

function combinationIndexFromId(value: string): bigint {
  const suffix = value.startsWith('COMBO-') ? value.slice('COMBO-'.length) : value;
  const oneBased = BigInt(suffix);
  return oneBased > 0n ? oneBased - 1n : 0n;
}

function valueText(value: { type: string; value?: boolean | number | string } | null | undefined) {
  if (!value || value.type === 'unknown') return 'UNKNOWN';
  if (value.type === 'boolean') return value.value ? 'ON' : 'OFF';
  return String(value.value);
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function statusClass(status: ReviewStatus) {
  switch (status) {
    case 'expected_by_design': return 'accepted';
    case 'needs_correction': return 'correction';
    case 'deferred': return 'deferred';
    default: return 'pending';
  }
}
