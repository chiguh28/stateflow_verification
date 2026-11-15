import { PageHeading, PanelHeader } from '../../components/ui';
import { useUiStore } from '../../app/uiStore';

export function ExecutionSettingsPage(props: { onOpenGraph: () => void }) {
  const graphNodes = useUiStore((state) => state.graphNodes);
  const project = useUiStore((state) => state.project);
  const order = [...graphNodes].sort((left, right) => left.position.x - right.position.x);

  if (!project) {
    return <section className="page-section"><PageHeading eyebrow="EXECUTION ORDER" title="機能の逐次実行設定" subtitle="プロジェクトを開くと実行順序を表示します。" /><div className="card empty-project">STATEFLOWプロジェクトがありません。</div></section>;
  }

  return (
    <section className="page-section">
      <PageHeading eyebrow="EXECUTION ORDER" title="機能の逐次実行設定" subtitle="依存関係エディタでの左→右のノード位置から実行順序を自動決定します。後続機能は最新状態を参照します。" actions={<span className="chip">SEQUENTIAL</span>} />
      <section className="card execution-card">
        <PanelHeader title="実行順序" trailing={<span className="chip">ノード位置が正</span>} />
        <div className="execution-order-list">{order.map((node, index) => {
          const machine = project.machines.find((candidate) => candidate.id === node.id);
          return <div className="execution-order-row" key={node.id}>
            <div><span className="sequence-number">{String(index + 1).padStart(2, '0')}</span><strong>{machine?.name ?? node.data.machineName}</strong><small>左→右の位置 x={Math.round(node.position.x)} ｜ 評価は1回 ｜ 後続は最新状態を参照</small></div>
            <span className="chip">{index === 0 ? 'FIRST' : index === order.length - 1 ? 'LAST' : 'NEXT'}</span>
          </div>;
        })}</div>
      </section>
      <button className="button dark open-graph-button" onClick={props.onOpenGraph} type="button">ノード配置で順序を変更 →</button>
      <div className="small-summary"><strong>検証契約：</strong>全機能を必ず1回評価し、成立しない場合は現在状態を維持します。同じ機能内で複数ルールが成立する場合は、数値の小さいpriorityを採用します。同順位で異なる次状態が成立したときは競合として報告します。</div>
      <p className="footer-hint">順序は保存済みプロジェクトのグラフノード位置から導きます。ここでは読み取り専用です。</p>
    </section>
  );
}
