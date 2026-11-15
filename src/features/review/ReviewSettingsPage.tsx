import { useState } from 'react';
import { PageHeading, PanelHeader } from '../../components/ui';
import { useUiStore, type ReviewRuleKey } from '../../app/uiStore';
import { invokeCommand, type ProjectResponse, type ReviewRuleId } from '../../lib/tauri';

const reviewRuleOptions: { key: ReviewRuleKey; id: string; title: string; detail: string }[] = [
  { key: 'upstream', id: 'R01', title: '上流の状態が変化したのに、参照する機能が状態維持', detail: '同じ1ステップで参照元が変化したのに依存先が変化しないパターン。正常な条件抑止の可能性があります。' },
  { key: 'previous', id: 'R02', title: '前回状態/前回出力の条件によって連動が抑止', detail: 'PREVの値だけを変えたときにガード成立可能性が変わる根拠がある場合に抽出します。' },
  { key: 'order', id: 'R03', title: '実行順序による未反映', detail: '対象機能の評価後に参照元が変化し、今回のステップでは再評価されない場合に抽出します。' },
  { key: 'implicit', id: 'R04', title: '暗黙の状態維持をすべて抽出', detail: 'どの遷移ルールも成立しなかった状態維持。候補数が多くなるため通常はOFFです。' },
  { key: 'allhold', id: 'R05', title: 'スイッチ操作に対して全機能が変化しない', detail: '入力が変化しても統合状態全体が同じままだった組み合わせを抽出します。' },
];

const ruleIds: Record<ReviewRuleKey, ReviewRuleId> = {
  upstream: 'upstream_changed_downstream_held',
  previous: 'previous_value_suppressed',
  order: 'order_missed_update',
  implicit: 'implicit_hold',
  allhold: 'all_machines_held',
};

export function ReviewSettingsPage(props: { onApplied: () => void }) {
  const project = useUiStore((state) => state.project);
  const savedPath = useUiStore((state) => state.savedPath);
  const setProject = useUiStore((state) => state.setProject);
  const setDirty = useUiStore((state) => state.setDirty);
  const appliedRules = useUiStore((state) => state.reviewRules);
  const appliedMachines = useUiStore((state) => state.enabledMachines);
  const appliedEvents = useUiStore((state) => state.enabledEvents);
  const [rules, setRules] = useState(appliedRules);
  const [machines, setMachines] = useState(appliedMachines);
  const [events, setEvents] = useState(appliedEvents);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  function resetSettings() {
    if (!project) return;
    setRules({ upstream: true, previous: true, order: true, implicit: false, allhold: false });
    setMachines(project.machines.map((machine) => machine.id));
    setEvents(project.events.map((event) => event.id));
  }

  async function applySettings() {
    if (!project) return;
    setBusy(true);
    setError('');
    try {
      const enabled_rules = Object.entries(rules).filter(([, enabled]) => enabled).map(([key]) => ruleIds[key as ReviewRuleKey]);
      const result = await invokeCommand<ProjectResponse>('update_review_settings', {
        settings: { enabled_rules, enabled_machines: machines, enabled_events: events },
        baseRevision: project.revision,
      });
      setProject(result.project, result.diagnostics, savedPath);
      setDirty(true);
      props.onApplied();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  if (!project) {
    return <section className="page-section"><PageHeading eyebrow="REVIEW CRITERIA" title="連動レビュー候補の抽出設定" subtitle="プロジェクトを開くと抽出条件を設定できます。" /><div className="card empty-project">STATEFLOWプロジェクトがありません。</div></section>;
  }

  return (
    <section className="page-section">
      <PageHeading eyebrow="REVIEW CRITERIA" title="連動レビュー候補の抽出設定" subtitle="どんな変化・状態維持をレビュー対象として一覧に出すかを設定します。"
        actions={<><button className="button" disabled={busy} onClick={resetSettings} type="button">初期設定に戻す</button><button className="button primary" disabled={busy} onClick={() => void applySettings()} type="button">{busy ? '保存中…' : '設定を反映して候補を表示 →'}</button></>} />
      {error ? <div className="diagnostics-banner" role="alert"><strong>設定を保存できません</strong><span>{error}</span></div> : null}
      <div className="settings-layout">
        <section className="card settings-block">
          <PanelHeader title="01 / 検出ルール" trailing={<span className="chip">複数選択</span>} />
          <div className="settings-options">
            {reviewRuleOptions.map((option) => <label className="review-option" key={option.key}>
              <input checked={rules[option.key]} onChange={(event) => setRules((currentRules) => ({ ...currentRules, [option.key]: event.target.checked }))} type="checkbox" />
              <span><strong>{option.title}</strong><small>{option.detail}</small></span><em>{option.id}</em>
            </label>)}
          </div>
        </section>
        <div className="settings-side">
          <section className="card settings-block">
            <PanelHeader title="02 / 対象機能" trailing={<span className="chip">機能別</span>} />
            <div className="settings-options compact-options">{project.machines.map((machine) => <label className="review-option" key={machine.id}>
              <input checked={machines.includes(machine.id)} onChange={() => setMachines(toggleId(machines, machine.id))} type="checkbox" />
              <span><strong>{machine.name}</strong><small>{machine.id} / {project.states.filter((state) => state.machine_id === machine.id).map((state) => state.id).join('・')}</small></span>
            </label>)}</div>
          </section>
          <section className="card settings-block">
            <PanelHeader title="03 / 対象イベント" trailing={<span className="chip">入力変化</span>} />
            <div className="settings-options compact-options">{project.events.map((event) => <label className="review-option" key={event.id}>
              <input checked={events.includes(event.id)} onChange={() => setEvents(toggleId(events, event.id))} type="checkbox" />
              <span><strong>{event.name}</strong><small>{event.from_value.type === 'boolean' ? (event.from_value.value ? 'ON' : 'OFF') : event.from_value.type === 'unknown' ? 'UNKNOWN' : event.from_value.value} → {event.to_value.type === 'boolean' ? (event.to_value.value ? 'ON' : 'OFF') : event.to_value.type === 'unknown' ? 'UNKNOWN' : event.to_value.value}</small></span>
            </label>)}</div>
          </section>
        </div>
      </div>
      <div className="small-summary"><strong>抽出対象について：</strong>チェックを外すとレビュー候補一覧とマトリクスの強調表示から除外されます。状態遷移そのものの評価ロジックや全組み合わせは変えません。検出候補は不具合の断定ではありません。</div>
      <p className="footer-hint">設定は現在のプロジェクトに保存されます。遷移評価と全状態組み合わせは変えず、レビュー候補の抽出対象だけを変えます。</p>
    </section>
  );
}

function toggleId(currentIds: string[], id: string) {
  return currentIds.includes(id) ? currentIds.filter((currentId) => currentId !== id) : [...currentIds, id];
}
