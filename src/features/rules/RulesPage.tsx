import { useMemo, useState } from 'react';
import { PageHeading, PanelHeader } from '../../components/ui';
import { useUiStore } from '../../app/uiStore';

export function RulesPage() {
  const project = useUiStore((state) => state.project);
  const [search, setSearch] = useState('');
  const [machineFilter, setMachineFilter] = useState('all');
  const visibleRules = useMemo(() => (project?.transitions ?? []).filter((rule) => {
    const matchesMachine = machineFilter === 'all' || rule.machine_id === machineFilter;
    const searchableText = `${rule.id} ${rule.machine_id} ${rule.from_state} ${rule.guard_source} ${rule.to_state} ${rule.source_ref ?? ''}`.toLowerCase();
    return matchesMachine && searchableText.includes(search.trim().toLowerCase());
  }).sort((left, right) => left.machine_id.localeCompare(right.machine_id) || left.priority - right.priority || left.id.localeCompare(right.id)), [machineFilter, project, search]);

  if (!project) {
    return <section className="page-section"><PageHeading eyebrow="RULES / EXCEL INPUT" title="標準化した状態遷移ルール" subtitle="Excelプロジェクトを開くとルールを表示します。" /><div className="card empty-project">STATEFLOWプロジェクトがありません。</div></section>;
  }

  return (
    <section className="page-section">
      <PageHeading eyebrow="RULES / EXCEL INPUT" title="標準化した状態遷移ルール" subtitle="1行=1遷移ルール。priorityの小さい順に評価し、検証済みのガード式を表示します。" actions={<span className="chip">{project.transitions.length.toLocaleString()} RULES</span>} />
      <div className="card rule-filters">
        <label className="field search-field"><span>ルール検索</span><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="機能・ID・条件・出典" /></label>
        <label className="field"><span>対象機能</span><select value={machineFilter} onChange={(event) => setMachineFilter(event.target.value)}><option value="all">すべての機能</option>{project.machines.map((machine) => <option key={machine.id} value={machine.id}>{machine.name}</option>)}</select></label>
        <span className="filter-count">{visibleRules.length.toLocaleString()} / {project.transitions.length.toLocaleString()} ルール</span>
      </div>
      <section className="card rules-card">
        <PanelHeader title="遷移ルール一覧" trailing={<span className="chip">priorityの小さい順</span>} />
        <div className="rules-table-scroll"><table className="rules-table">
          <thead><tr><th>priority</th><th>機能</th><th>現在状態</th><th>guard</th><th>次状態</th><th>source_ref</th></tr></thead>
          <tbody>{visibleRules.map((rule) => {
            const machine = project.machines.find((candidate) => candidate.id === rule.machine_id);
            return <tr key={rule.id}>
              <td><span className="priority-badge">{rule.priority}</span></td>
              <td><strong>{rule.machine_id}</strong><small>{machine?.name ?? rule.machine_id}</small></td>
              <td>{rule.from_state}</td>
              <td><code>{highlightReferences(rule.guard_source)}</code><small>{rule.id}</small></td>
              <td><strong>{rule.to_state}</strong></td>
              <td className="source-ref">{rule.source_ref ?? (rule.source ? `${rule.source.file_name} / ${rule.source.sheet}!${rule.source.column ?? ''}${rule.source.row}` : '—')}</td>
            </tr>;
          })}{visibleRules.length === 0 ? <tr><td className="empty-table" colSpan={6}>該当するルールがありません。</td></tr> : null}</tbody>
        </table></div>
      </section>
      <div className="small-summary"><strong>履歴参照の定義：</strong> <code>STATE.MACHINE</code> は逐次評価で更新された最新状態、<code>BEFORE.STATE.MACHINE</code> は今回の評価開始状態、<code>PREV.STATE.MACHINE</code> は前回確定ステップの状態、<code>PREV.OUTPUT.SIGNAL</code> は前回確定ステップの出力です。</div>
      <p className="footer-hint">対象：{project.machines.length}機能 / {project.events.length}イベント / {project.transitions.length.toLocaleString()}遷移ルール</p>
    </section>
  );
}

function highlightReferences(source: string) {
  return source.split(/(STATE\.[A-Z0-9_]+|BEFORE\.STATE\.[A-Z0-9_]+|PREV\.STATE\.[A-Z0-9_]+|PREV\.OUTPUT\.[A-Z0-9_]+|INPUT\.[A-Z0-9_]+)/g).map((part, index) => <span className={part.includes('.') ? 'guard-reference' : undefined} key={`${index}-${part}`}>{part}</span>);
}
