export const demoMachines = [
  { id: 'AC', name: '空調制御', shortName: '空調', states: ['MANUAL', 'AUTO'] },
  { id: 'LINK', name: '連携制御', shortName: '連携', states: ['OFF', 'ON'] },
  { id: 'HEATER', name: 'ヒーター制御', shortName: 'ヒーター', states: ['OFF', 'ON'] },
] as const;

export type DemoMachineId = (typeof demoMachines)[number]['id'];
export type DemoPageId = 'matrix' | 'reviews' | 'review-settings' | 'graph' | 'rules' | 'settings';

export type DemoCombination = {
  id: string;
  states: Record<DemoMachineId, string>;
};

export const demoCombinations: DemoCombination[] = [
  { id: 'COMBO-01', states: { AC: 'MANUAL', LINK: 'OFF', HEATER: 'OFF' } },
  { id: 'COMBO-02', states: { AC: 'MANUAL', LINK: 'OFF', HEATER: 'ON' } },
  { id: 'COMBO-03', states: { AC: 'MANUAL', LINK: 'ON', HEATER: 'OFF' } },
  { id: 'COMBO-04', states: { AC: 'MANUAL', LINK: 'ON', HEATER: 'ON' } },
  { id: 'COMBO-05', states: { AC: 'AUTO', LINK: 'OFF', HEATER: 'OFF' } },
  { id: 'COMBO-06', states: { AC: 'AUTO', LINK: 'OFF', HEATER: 'ON' } },
  { id: 'COMBO-07', states: { AC: 'AUTO', LINK: 'ON', HEATER: 'OFF' } },
  { id: 'COMBO-08', states: { AC: 'AUTO', LINK: 'ON', HEATER: 'ON' } },
];

export const demoEvents = [
  { id: 'AUTO_SW_UP', name: 'AUTOスイッチ ON', input: 'AUTO_SW', from: 'OFF', to: 'ON' },
  { id: 'AUTO_SW_DOWN', name: 'AUTOスイッチ OFF', input: 'AUTO_SW', from: 'ON', to: 'OFF' },
  { id: 'LINK_SW_UP', name: '連携スイッチ ON', input: 'LINK_SW', from: 'OFF', to: 'ON' },
  { id: 'LINK_SW_DOWN', name: '連携スイッチ OFF', input: 'LINK_SW', from: 'ON', to: 'OFF' },
  { id: 'HEATER_SW_UP', name: 'ヒータースイッチ ON', input: 'HEATER_SW', from: 'OFF', to: 'ON' },
] as const;

export type DemoEventId = (typeof demoEvents)[number]['id'];

// Fixed display-only values for the Phase 1 shell. Production results will come from stateflow-core.
export const demoMatrixRows: Record<DemoEventId, string[][]> = {
  AUTO_SW_UP: [
    ['AUTO', 'OFF', 'OFF'], ['AUTO', 'OFF', 'ON'], ['AUTO', 'ON', 'OFF'], ['AUTO', 'ON', 'ON'],
    ['AUTO', 'OFF', 'OFF'], ['AUTO', 'OFF', 'ON'], ['AUTO', 'ON', 'OFF'], ['AUTO', 'ON', 'ON'],
  ],
  AUTO_SW_DOWN: [
    ['MANUAL', 'OFF', 'OFF'], ['MANUAL', 'OFF', 'ON'], ['MANUAL', 'ON', 'OFF'], ['MANUAL', 'ON', 'ON'],
    ['MANUAL', 'OFF', 'OFF'], ['MANUAL', 'OFF', 'ON'], ['MANUAL', 'ON', 'OFF'], ['MANUAL', 'ON', 'ON'],
  ],
  LINK_SW_UP: [
    ['MANUAL', 'ON', 'OFF'], ['MANUAL', 'ON', 'ON'], ['MANUAL', 'ON', 'OFF'], ['MANUAL', 'ON', 'ON'],
    ['AUTO', 'ON', 'OFF'], ['AUTO', 'ON', 'ON'], ['AUTO', 'ON', 'ON'], ['AUTO', 'ON', 'ON'],
  ],
  LINK_SW_DOWN: [
    ['MANUAL', 'OFF', 'OFF'], ['MANUAL', 'OFF', 'ON'], ['MANUAL', 'OFF', 'OFF'], ['MANUAL', 'OFF', 'ON'],
    ['AUTO', 'OFF', 'OFF'], ['AUTO', 'OFF', 'ON'], ['AUTO', 'OFF', 'OFF'], ['AUTO', 'OFF', 'ON'],
  ],
  HEATER_SW_UP: [
    ['MANUAL', 'OFF', 'ON'], ['MANUAL', 'OFF', 'ON'], ['MANUAL', 'ON', 'ON'], ['MANUAL', 'ON', 'ON'],
    ['AUTO', 'OFF', 'ON'], ['AUTO', 'OFF', 'ON'], ['AUTO', 'ON', 'ON'], ['AUTO', 'ON', 'ON'],
  ],
};

export const demoRules = [
  { id: 'AC_001', priority: 1, machine: 'AC', from: 'MANUAL', guard: 'RISE(AUTO_SW)', to: 'AUTO', source: '空調仕様書 3.1 / Sheet1!D8' },
  { id: 'AC_002', priority: 1, machine: 'AC', from: 'AUTO', guard: 'FALL(AUTO_SW)', to: 'MANUAL', source: '空調仕様書 3.1 / Sheet1!D9' },
  { id: 'LINK_001', priority: 1, machine: 'LINK', from: 'OFF', guard: 'RISE(LINK_SW)', to: 'ON', source: '連携仕様書 2.4 / Sheet2!C12' },
  { id: 'LINK_002', priority: 1, machine: 'LINK', from: 'ON', guard: 'FALL(LINK_SW)', to: 'OFF', source: '連携仕様書 2.4 / Sheet2!C13' },
  { id: 'HEATER_001', priority: 1, machine: 'HEATER', from: 'ON', guard: 'STATE.AC == MANUAL OR STATE.LINK == OFF OR INPUT.SAFETY_LOCK == ON', to: 'OFF', source: '熱制御仕様書 4.2 / Sheet3!B21' },
  { id: 'HEATER_002', priority: 2, machine: 'HEATER', from: 'OFF', guard: 'RISE(HEATER_SW) AND INPUT.SAFETY_LOCK == OFF', to: 'ON', source: '熱制御仕様書 4.3 / Sheet3!B29' },
  { id: 'HEATER_003', priority: 3, machine: 'HEATER', from: 'OFF', guard: 'STATE.AC == AUTO AND STATE.LINK == ON AND PREV.OUTPUT.HEATER_CMD == OFF', to: 'ON', source: '熱制御仕様書 4.4 / Sheet3!B35' },
] as const;

export type DemoReview = {
  id: string;
  eventId: DemoEventId;
  combinationId: string;
  machineId: DemoMachineId;
  currentState: string;
  nextState: string;
  reasons: string[];
  sourceRule: string;
};

export const demoReviews: DemoReview[] = [
  { id: 'RV-0103-HEATER', eventId: 'AUTO_SW_UP', combinationId: 'COMBO-03', machineId: 'HEATER', currentState: 'OFF', nextState: 'OFF', reasons: ['上流状態が変化し、依存先は状態維持'], sourceRule: 'HEATER_003' },
  { id: 'RV-0104-HEATER', eventId: 'AUTO_SW_UP', combinationId: 'COMBO-04', machineId: 'HEATER', currentState: 'ON', nextState: 'ON', reasons: ['上流状態が変化し、依存先は状態維持'], sourceRule: 'HEATER_001' },
  { id: 'RV-0207-HEATER', eventId: 'AUTO_SW_DOWN', combinationId: 'COMBO-07', machineId: 'HEATER', currentState: 'OFF', nextState: 'OFF', reasons: ['前回出力が遷移条件を抑止'], sourceRule: 'HEATER_003' },
  { id: 'RV-0305-LINK', eventId: 'LINK_SW_UP', combinationId: 'COMBO-05', machineId: 'LINK', currentState: 'OFF', nextState: 'ON', reasons: ['入力変化を逐次評価'], sourceRule: 'LINK_001' },
] as const;

export const demoExecutionOrder: DemoMachineId[] = ['AC', 'LINK', 'HEATER'];

export const demoTrace = [
  { machineId: 'AC' as const, from: 'MANUAL', to: 'AUTO', status: 'CHANGED', detail: 'AC_001 が成立。priority 1を採用。' },
  { machineId: 'LINK' as const, from: 'OFF', to: 'ON', status: 'CHANGED', detail: 'LINK_001 が成立。priority 1を採用。' },
  { machineId: 'HEATER' as const, from: 'OFF', to: 'ON', status: 'CHANGED', detail: 'HEATER_003 が成立。前回出力 HEATER_CMD は OFF。' },
];
