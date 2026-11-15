import { useEffect, useMemo, useRef, useState } from 'react';
import {
  applyEdgeChanges,
  addEdge,
  Background,
  Controls,
  Handle,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  type Connection,
  type Edge,
  type NodeProps,
} from '@xyflow/react';
import { Chip, PageHeading, PanelHeader } from '../../components/ui';
import { useUiStore, type MachineNode, type UiEdge } from '../../app/uiStore';
import { invokeCommand, type GraphEdgeDto, type GraphNodeDto, type ProjectResponse } from '../../lib/tauri';

type EdgeData = { kind: 'CURRENT' | 'PREVIOUS' | 'FLOW_ONLY'; previousScope?: 'state' | 'output'; outputId?: string };
type DemoEdge = Edge<EdgeData>;

const edgeColors: Record<EdgeData['kind'], string> = {
  CURRENT: '#e82127',
  PREVIOUS: '#8995a4',
  FLOW_ONLY: '#f0f2f5',
};

function MachineNodeCard(props: NodeProps<MachineNode>) {
  return (
    <div className="machine-node">
      <Handle className="node-handle current" id="in-CURRENT" position={Position.Left} style={{ top: 94 }} type="target" />
      <Handle className="node-handle previous" id="in-PREVIOUS" position={Position.Left} style={{ top: 126 }} type="target" />
      <Handle className="node-handle flow" id="in-FLOW_ONLY" position={Position.Left} style={{ top: 158 }} type="target" />
      <div className="machine-node-header">
        <div className="node-name">{props.data.machineName}</div>
        <div className="node-id">{props.data.machineId} / {props.data.states.length} STATES</div>
      </div>
      <div className="machine-node-body">
        <div className="node-port-row"><i className="port-dot current" /><span>STATE / CURRENT</span><small>入力</small></div>
        <div className="node-port-row"><i className="port-dot previous" /><span>PREV / HISTORY</span><small>入力</small></div>
        <div className="node-port-row"><i className="port-dot flow" /><span>FLOW / ORDER</span><small>入力</small></div>
        <div className="node-state-list">{props.data.states.map((state) => <span key={state}>{state}</span>)}</div>
      </div>
      <Handle className="node-handle current" id="out-CURRENT" position={Position.Right} style={{ top: 94 }} type="source" />
      <Handle className="node-handle previous" id="out-PREVIOUS" position={Position.Right} style={{ top: 126 }} type="source" />
      <Handle className="node-handle flow" id="out-FLOW_ONLY" position={Position.Right} style={{ top: 158 }} type="source" />
    </div>
  );
}

const nodeTypes = { machine: MachineNodeCard };

export function GraphPage() {
  return <ReactFlowProvider><GraphEditor /></ReactFlowProvider>;
}

function GraphEditor() {
  const storedNodes = useUiStore((state) => state.graphNodes);
  const storedEdges = useUiStore((state) => state.graphEdges) as UiEdge[];
  const project = useUiStore((state) => state.project);
  const savedPath = useUiStore((state) => state.savedPath);
  const setProject = useUiStore((state) => state.setProject);
  const setDirty = useUiStore((state) => state.setDirty);
  const saveNodes = useUiStore((state) => state.setGraphNodes);
  const saveEdges = useUiStore((state) => state.setGraphEdges);
  const [nodes, setNodes, onNodesChange] = useNodesState<MachineNode>(storedNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState<UiEdge>(storedEdges);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [graphError, setGraphError] = useState('');
  const saveQueue = useRef(Promise.resolve());
  const selectedEdge = edges.find((edge) => edge.id === selectedEdgeId) ?? null;
  const executionOrder = useMemo(() => [...nodes].sort((left, right) => left.position.x - right.position.x), [nodes]);

  useEffect(() => {
    if (project) {
      setNodes(storedNodes);
      setEdges(storedEdges);
    }
  }, [project?.project_id, setEdges, setNodes, storedEdges, storedNodes]);
  useEffect(() => saveNodes(nodes), [nodes, saveNodes]);
  useEffect(() => saveEdges(edges), [edges, saveEdges]);

  function persistGraph(nextNodes: MachineNode[], nextEdges: UiEdge[]) {
    if (!project) return;
    const graphNodes: GraphNodeDto[] = nextNodes.map((node) => ({
      machine_id: node.id,
      x: node.position.x,
      y: node.position.y,
    }));
    const graphEdges: GraphEdgeDto[] = nextEdges.map((edge) => ({
      edge_id: edge.id,
      source_machine: edge.source,
      target_machine: edge.target,
      kind: edgeKindToDto(edge.data),
      output_id: edge.data?.kind === 'PREVIOUS' && edge.data.previousScope === 'output' ? edge.data.outputId ?? null : null,
      target_reference: null,
    }));
    saveQueue.current = saveQueue.current.then(async () => {
      try {
        const result = await invokeCommand<ProjectResponse>('update_graph', {
          nodes: graphNodes,
          edges: graphEdges,
          baseRevision: useUiStore.getState().project?.revision ?? project.revision,
        });
        setProject(result.project, result.diagnostics, savedPath);
        setDirty(true);
        setGraphError('');
      } catch (error) {
        setGraphError(error instanceof Error ? error.message : String(error));
      }
    });
  }

  function validateConnection(connection: Connection | DemoEdge) {
    if (!connection.source || !connection.target || connection.source === connection.target) return false;
    const sourceNode = nodes.find((node) => node.id === connection.source);
    const targetNode = nodes.find((node) => node.id === connection.target);
    if (!sourceNode || !targetNode || sourceNode.position.x >= targetNode.position.x) return false;
    const sourceKind = connection.sourceHandle?.replace('out-', '');
    const targetKind = connection.targetHandle?.replace('in-', '');
    return Boolean(sourceKind && sourceKind === targetKind);
  }

  function connect(connection: Connection) {
    if (!validateConnection(connection)) return;
    const edgeKind = connection.sourceHandle?.replace('out-', '') as EdgeData['kind'];
    const newEdge: UiEdge = {
      ...connection,
      id: `edge-${crypto.randomUUID()}`,
      type: 'smoothstep',
      label: edgeKind,
      data: { kind: edgeKind, previousScope: 'state' },
      style: {
        stroke: edgeColors[edgeKind],
        strokeWidth: 3,
        ...(edgeKind === 'PREVIOUS' ? { strokeDasharray: '7 6' } : {}),
      },
      markerEnd: { type: 'arrowclosed', color: edgeColors[edgeKind] },
    };
    const updatedEdges = addEdge(newEdge, edges);
    setEdges(updatedEdges);
    persistGraph(nodes, updatedEdges);
    setSelectedEdgeId(newEdge.id);
  }

  function updateSelectedEdge(update: (edge: UiEdge) => UiEdge) {
    const updatedEdges = edges.map((edge) => edge.id === selectedEdgeId ? update(edge) : edge);
    setEdges(updatedEdges);
    persistGraph(nodes, updatedEdges);
  }

  if (!project) {
    return <section className="page-section"><PageHeading eyebrow="DEPENDENCY + EXECUTION EDITOR" title="依存関係・実行順序エディタ" subtitle="Excelプロジェクトを開くと機能ノードを表示します。" /><div className="card empty-project">STATEFLOWプロジェクトがありません。</div></section>;
  }

  return (
    <section className="page-section graph-page">
      <PageHeading eyebrow="DEPENDENCY + EXECUTION EDITOR" title="依存関係・実行順序エディタ" subtitle="ComfyUI方式。ノードは自動作成、接続はユーザーが手動で行います。実行は左から右へ進みます。"
        actions={<><Chip>{edges.length} CONNECTIONS</Chip><button className="button small" onClick={() => { setEdges([]); setSelectedEdgeId(null); persistGraph(nodes, []); }} type="button">全接続を解除</button></>} />
      {graphError ? <div className="diagnostics-banner" role="alert"><strong>グラフを保存できません</strong><span>{graphError}</span></div> : null}
      <div className="graph-disclaimer"><strong>接続線には2つの意味があります：</strong> 赤（現在状態）・グレー（前回状態）の接続は <b>①入出力の依存関係</b> と <b>②実行順序</b> を表します。白（実行フロー）は実行順序だけを指定します。ノードは取込後も自動接続されません。</div>
      <div className="graph-legend"><span><i className="legend-line current" /> 現在状態参照 ＋ 順序</span><span><i className="legend-line previous" /> 前回状態/出力参照 ＋ 順序</span><span><i className="legend-line flow" /> 実行順序のみ</span></div>
      <div className="graph-layout">
        <div className="graph-canvas">
          <div className="graph-order-floater"><small>LIVE EXECUTION ORDER / X-AXIS</small><div>{executionOrder.map((node) => node.data.machineId).join('  →  ')}</div></div>
          <div className="graph-status">右側の出力ポートから左側の入力ポートへドラッグして接続</div>
          <ReactFlow
            nodes={nodes}
            edges={edges}
            nodeTypes={nodeTypes}
            onNodesChange={onNodesChange}
            onNodeDragStop={(_event, movedNode) => persistGraph(nodes.map((node) => node.id === movedNode.id ? movedNode : node), edges)}
            onEdgesChange={(changes) => {
              const updatedEdges = applyEdgeChanges(changes, edges);
              onEdgesChange(changes);
              if (changes.some((change) => change.type === 'remove')) {
                setSelectedEdgeId(null);
                persistGraph(nodes, updatedEdges);
              }
            }}
            onConnect={connect}
            isValidConnection={validateConnection}
            onEdgeClick={(_event, edge) => setSelectedEdgeId(edge.id)}
            onPaneClick={() => setSelectedEdgeId(null)}
            fitView
            fitViewOptions={{ padding: 0.15 }}
            minZoom={0.35}
            maxZoom={1.6}
            deleteKeyCode={['Backspace', 'Delete']}
          >
            <Background color="#45494e" gap={20} size={1} />
            <Controls position="bottom-right" showInteractive={false} />
          </ReactFlow>
        </div>

        <aside className="card graph-inspector">
          <PanelHeader title="接続と実行ルール" trailing={<Chip>USER DEFINED</Chip>} />
          <div className="connection-list">
            {edges.length === 0 ? <div className="connection-empty">まだ接続はありません</div> : edges.map((edge) => {
              const edgeData = edge.data as EdgeData;
              const sourceName = nodes.find((node) => node.id === edge.source)?.data.machineId ?? edge.source;
              const targetName = nodes.find((node) => node.id === edge.target)?.data.machineId ?? edge.target;
              return <div className={`connection-row${edge.id === selectedEdgeId ? ' selected' : ''}`} key={edge.id}>
                <button className="connection-select" onClick={() => setSelectedEdgeId(edge.id)} type="button"><i style={{ background: edgeColors[edgeData.kind] }} />{sourceName} → {targetName}<small>{edgeData.kind}</small></button>
                <button className="remove-edge" aria-label={`${sourceName}から${targetName}への接続を削除`} onClick={() => { const updatedEdges = edges.filter((candidate) => candidate.id !== edge.id); setEdges(updatedEdges); persistGraph(nodes, updatedEdges); if (selectedEdgeId === edge.id) setSelectedEdgeId(null); }} type="button">削除</button>
              </div>;
            })}
          </div>
          <div className="connection-detail">
            <h3>接続の詳細</h3>
            {selectedEdge ? <>
              <p><strong>データ参照：</strong>{selectedEdge.data?.kind === 'FLOW_ONLY' ? 'なし（実行順序のみ）' : `${selectedEdge.source} → ${selectedEdge.target}`}</p>
              <p><strong>実行順序：</strong>{selectedEdge.source} を先に評価</p>
              {selectedEdge.data?.kind === 'PREVIOUS' ? <>
                <label className="field"><span>履歴参照の種類</span><select value={selectedEdge.data.previousScope ?? 'state'} onChange={(event) => updateSelectedEdge((edge) => ({ ...edge, data: { ...(edge.data ?? { kind: 'PREVIOUS' }), previousScope: event.target.value as 'state' | 'output' } }))}><option value="state">前回状態（PREV.STATE）</option><option value="output">前回出力（PREV.OUTPUT）</option></select></label>
                {selectedEdge.data.previousScope === 'output' ? <label className="field"><span>出力信号ID</span><select value={selectedEdge.data.outputId ?? ''} onChange={(event) => updateSelectedEdge((edge) => ({ ...edge, data: { ...(edge.data ?? { kind: 'PREVIOUS' }), previousScope: 'output', outputId: event.target.value } }))}><option value="">出力を選択</option>{project?.outputs.filter((output) => output.owner_machine_id === selectedEdge.source).map((output) => <option key={output.id} value={output.id}>{output.name} / {output.id}</option>)}</select></label> : null}
              </> : null}
            </> : <p className="note">線を接続または選択すると、その接続が渡すデータと実行順序が表示されます。</p>}
          </div>
          <div className="graph-help">● 出力ポートをドラッグして同じ種類の入力へ接続<br />● ノードの位置が実行順序。左→右へ配置<br />● 接続を選択して削除可能<br />● PREVIOUSの前回値は今回の評価で書き換わりません<br />● 未接続ノードも左から右へ各1回評価します</div>
        </aside>
      </div>
      <p className="footer-hint">接続は手動で設定します。ガード式の参照と依存関係はRustコアが解析前に検証します。</p>
    </section>
  );
}

function edgeKindToDto(data: UiEdge['data']): GraphEdgeDto['kind'] {
  if (data?.kind === 'CURRENT') return 'current_state';
  if (data?.kind === 'FLOW_ONLY') return 'flow_only';
  return data?.previousScope === 'output' ? 'previous_output' : 'previous_state';
}
