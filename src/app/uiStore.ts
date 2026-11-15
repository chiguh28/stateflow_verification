import { create } from 'zustand';
import type { Edge, Node } from '@xyflow/react';
import type { DemoPageId } from '../fixtures/uiDemo';
import type { Diagnostic, GraphEdgeDto, ProjectDto } from '../lib/tauri';

export type MachineNodeData = {
  machineId: string;
  machineName: string;
  states: readonly string[];
};

export type MachineNode = Node<MachineNodeData, 'machine'>;
export type UiEdgeData = { kind: 'CURRENT' | 'PREVIOUS' | 'FLOW_ONLY'; previousScope?: 'state' | 'output'; outputId?: string };
export type UiEdge = Edge<UiEdgeData>;

export type ReviewRuleKey = 'upstream' | 'previous' | 'order' | 'implicit' | 'allhold';

type UiState = {
  activePage: DemoPageId;
  graphNodes: MachineNode[];
  graphEdges: Edge[];
  reviewRules: Record<ReviewRuleKey, boolean>;
  enabledMachines: string[];
  enabledEvents: string[];
  project: ProjectDto | null;
  diagnostics: Diagnostic[];
  savedPath: string | null;
  dirty: boolean;
  matrixFocus: { eventId: string; combinationIndex: string; machineId: string } | null;
  reviewCandidateCount: number | null;
  setActivePage: (page: DemoPageId) => void;
  setGraphNodes: (nodes: MachineNode[]) => void;
  setGraphEdges: (edges: Edge[]) => void;
  setReviewRules: (rules: Record<ReviewRuleKey, boolean>) => void;
  setEnabledMachines: (machineIds: string[]) => void;
  setEnabledEvents: (eventIds: string[]) => void;
  setProject: (project: ProjectDto, diagnostics?: Diagnostic[], savedPath?: string | null) => void;
  setDiagnostics: (diagnostics: Diagnostic[]) => void;
  setSavedPath: (savedPath: string | null) => void;
  setDirty: (dirty: boolean) => void;
  setMatrixFocus: (focus: UiState['matrixFocus']) => void;
  setReviewCandidateCount: (count: number | null) => void;
  clearProject: () => void;
};

export const useUiStore = create<UiState>((set) => ({
  activePage: 'matrix',
  graphNodes: [],
  graphEdges: [],
  reviewRules: { upstream: true, previous: true, order: true, implicit: false, allhold: false },
  enabledMachines: [],
  enabledEvents: [],
  project: null,
  diagnostics: [],
  savedPath: null,
  dirty: false,
  matrixFocus: null,
  reviewCandidateCount: null,
  setActivePage: (activePage) => set({ activePage }),
  setGraphNodes: (graphNodes) => set({ graphNodes }),
  setGraphEdges: (graphEdges) => set({ graphEdges }),
  setReviewRules: (reviewRules) => set({ reviewRules }),
  setEnabledMachines: (enabledMachines) => set({ enabledMachines }),
  setEnabledEvents: (enabledEvents) => set({ enabledEvents }),
  setProject: (project, diagnostics = [], savedPath = null) => set({
    project,
    diagnostics,
    savedPath,
    dirty: false,
    reviewCandidateCount: null,
    graphNodes: projectToGraphNodes(project),
    graphEdges: project.graph_edges.map(projectEdgeToUiEdge),
    enabledMachines: project.review_settings.enabled_machines,
    enabledEvents: project.review_settings.enabled_events,
    reviewRules: projectReviewRules(project),
  }),
  setDiagnostics: (diagnostics) => set({ diagnostics }),
  setSavedPath: (savedPath) => set({ savedPath }),
  setDirty: (dirty) => set({ dirty }),
  setMatrixFocus: (matrixFocus) => set({ matrixFocus }),
  setReviewCandidateCount: (reviewCandidateCount) => set({ reviewCandidateCount }),
  clearProject: () => set({ project: null, diagnostics: [], savedPath: null, dirty: false, reviewCandidateCount: null }),
}));

function projectToGraphNodes(project: ProjectDto): MachineNode[] {
  const stateIdsByMachine = new Map<string, string[]>();
  for (const state of project.states) {
    const stateIds = stateIdsByMachine.get(state.machine_id) ?? [];
    stateIds.push(state.id);
    stateIdsByMachine.set(state.machine_id, stateIds);
  }
  return project.graph_nodes.map((node) => {
    const machine = project.machines.find((candidate) => candidate.id === node.machine_id);
    return {
      id: node.machine_id,
      type: 'machine',
      position: { x: node.x, y: node.y },
      data: {
        machineId: node.machine_id,
        machineName: machine?.name ?? node.machine_id,
        states: stateIdsByMachine.get(node.machine_id) ?? [],
      },
    };
  });
}

export function projectEdgeToUiEdge(edge: GraphEdgeDto): UiEdge {
  const kind: UiEdgeData['kind'] = edge.kind === 'current_state'
    ? 'CURRENT'
    : edge.kind === 'flow_only' ? 'FLOW_ONLY' : 'PREVIOUS';
  const previousScope = edge.kind === 'previous_output' ? 'output' : 'state';
  const color = kind === 'CURRENT' ? '#e82127' : kind === 'PREVIOUS' ? '#8995a4' : '#f0f2f5';
  return {
    id: edge.edge_id,
    source: edge.source_machine,
    target: edge.target_machine,
    sourceHandle: `out-${kind}`,
    targetHandle: `in-${kind}`,
    type: 'smoothstep',
    label: kind === 'PREVIOUS' && previousScope === 'output' ? 'PREV.OUTPUT' : kind,
    data: { kind, previousScope, outputId: edge.output_id ?? undefined },
    style: { stroke: color, strokeWidth: 3, ...(kind === 'PREVIOUS' ? { strokeDasharray: '7 6' } : {}) },
    markerEnd: { type: 'arrowclosed', color },
  };
}

function projectReviewRules(project: ProjectDto): Record<ReviewRuleKey, boolean> {
  return {
    upstream: project.review_settings.enabled_rules.includes('upstream_changed_downstream_held'),
    previous: project.review_settings.enabled_rules.includes('previous_value_suppressed'),
    order: project.review_settings.enabled_rules.includes('order_missed_update'),
    implicit: project.review_settings.enabled_rules.includes('implicit_hold'),
    allhold: project.review_settings.enabled_rules.includes('all_machines_held'),
  };
}
