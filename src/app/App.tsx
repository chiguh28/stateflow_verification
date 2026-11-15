import { useState } from 'react';
import type { DemoPageId } from '../fixtures/uiDemo';
import { Chip } from '../components/ui';
import { useUiStore } from './uiStore';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import {
  getMatrixMetadata,
  invokeCommand,
  isDesktopApp,
  type ImportResult,
  type ProjectResponse,
} from '../lib/tauri';
import { MatrixPage } from '../features/matrix/MatrixPage';
import { ReviewPage } from '../features/review/ReviewPage';
import { ReviewSettingsPage } from '../features/review/ReviewSettingsPage';
import { GraphPage } from '../features/graph/GraphPage';
import { RulesPage } from '../features/rules/RulesPage';
import { ExecutionSettingsPage } from '../features/execution/ExecutionSettingsPage';

const pages: { id: DemoPageId; label: string; title: string; icon: string; group: string }[] = [
  { id: 'matrix', label: '統合マトリクス', title: '統合状態遷移マトリクス', icon: '▦', group: 'WORKSPACE' },
  { id: 'reviews', label: '連動レビュー候補', title: '連動レビュー候補一覧', icon: '⚑', group: 'WORKSPACE' },
  { id: 'review-settings', label: 'レビュー抽出設定', title: '連動レビュー候補の抽出設定', icon: '⚙', group: 'WORKSPACE' },
  { id: 'graph', label: '依存関係・実行順序', title: '依存関係・実行順序エディタ', icon: '⌘', group: 'GRAPH' },
  { id: 'rules', label: '遷移ルール', title: '標準化した状態遷移ルール', icon: '≡', group: 'MODEL' },
  { id: 'settings', label: '実行設定', title: '機能の逐次実行設定', icon: '↗', group: 'MODEL' },
];

export function App() {
  const activePage = useUiStore((state) => state.activePage);
  const setActivePage = useUiStore((state) => state.setActivePage);
  const project = useUiStore((state) => state.project);
  const diagnostics = useUiStore((state) => state.diagnostics);
  const savedPath = useUiStore((state) => state.savedPath);
  const dirty = useUiStore((state) => state.dirty);
  const reviewCandidateCount = useUiStore((state) => state.reviewCandidateCount);
  const setProject = useUiStore((state) => state.setProject);
  const setSavedPath = useUiStore((state) => state.setSavedPath);
  const setDirty = useUiStore((state) => state.setDirty);
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const activePageDetails = pages.find((page) => page.id === activePage) ?? pages[0];

  function showNotice(message: string) {
    setNotice(message);
    window.setTimeout(() => setNotice(''), 5000);
  }

  async function runAction(action: () => Promise<void>) {
    setBusy(true);
    try {
      await action();
    } catch (error) {
      showNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function openProject() {
    const path = await openDialog({
      multiple: false,
      filters: [{ name: 'STATEFLOW Project', extensions: ['json'] }],
    });
    if (!path || Array.isArray(path)) return;
    const result = await invokeCommand<ProjectResponse>('open_project', { path });
    setProject(result.project, result.diagnostics, path);
    showNotice(`プロジェクト「${result.project.name}」を開きました。`);
  }

  async function importExcel() {
    const paths = await openDialog({
      multiple: true,
      filters: [{ name: 'Excel workbook', extensions: ['xlsx', 'xlsm', 'xls'] }],
    });
    if (!paths || typeof paths === 'string') return;
    const result = await invokeCommand<ImportResult>('import_excel_files', { paths });
    setProject(result.project, result.diagnostics);
    const errors = result.diagnostics.filter((diagnostic) => diagnostic.severity === 'error').length;
    showNotice(errors === 0
      ? `${result.project.machines.length}機能、${result.project.transitions.length}ルールを読み込みました。接続は依存関係画面で手動設定してください。`
      : `Excelを読み込みました。${errors}件の診断を確認してください。`);
  }

  async function saveProject() {
    if (!project) {
      showNotice('先にExcelを取り込むか、プロジェクトを開いてください。');
      return;
    }
    const path = savedPath ?? await saveDialog({
      defaultPath: `${project.name}.stateflow.json`,
      filters: [{ name: 'STATEFLOW Project', extensions: ['stateflow.json'] }],
    });
    if (!path) return;
    const saved = await invokeCommand<string>('save_project', { path });
    setSavedPath(saved);
    setDirty(false);
    showNotice('プロジェクトを保存しました。');
  }

  async function exportMatrix() {
    if (!project) {
      showNotice('先にプロジェクトを開いてください。');
      return;
    }
    const metadata = await getMatrixMetadata();
    const combinationCount = BigInt(metadata.combination_count);
    if (combinationCount > BigInt(Number.MAX_SAFE_INTEGER)) {
      showNotice('状態組み合わせ数が大きいため、マトリクス画面で範囲を絞ってCSVを出力してください。');
      return;
    }
    const path = await saveDialog({
      defaultPath: `${project.name}-matrix.csv`,
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    });
    if (!path) return;
    const eventCount = project.events.filter((event) => event.enabled).length;
    const exported = await invokeCommand<number>('export_matrix_csv', {
      path,
      eventStart: 0,
      eventCount,
      combinationStart: '0',
      combinationCount: Number(combinationCount),
      revision: metadata.revision,
    });
    showNotice(`${exported.toLocaleString()}セルをCSVに出力しました。`);
  }

  async function reevaluate() {
    if (!project) {
      showNotice('先にExcelを取り込むか、プロジェクトを開いてください。');
      return;
    }
    const metadata = await getMatrixMetadata();
    showNotice(`${metadata.event_count}イベント × ${metadata.combination_count}状態組み合わせで再評価できます。`);
  }

  function renderCurrentPage() {
    switch (activePage) {
      case 'matrix':
        return <MatrixPage onOpenReviews={() => setActivePage('reviews')} />;
      case 'reviews':
        return <ReviewPage onShowMatrix={() => setActivePage('matrix')} onOpenSettings={() => setActivePage('review-settings')} />;
      case 'review-settings':
        return <ReviewSettingsPage onApplied={() => setActivePage('reviews')} />;
      case 'graph':
        return <GraphPage />;
      case 'rules':
        return <RulesPage />;
      case 'settings':
        return <ExecutionSettingsPage onOpenGraph={() => setActivePage('graph')} />;
      default:
        return <MatrixPage onOpenReviews={() => setActivePage('reviews')} />;
    }
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">S</div>
          <div>
            <div className="brand-name">STATEFLOW</div>
            <div className="brand-subtitle">STATE LINKAGE ANALYZER</div>
          </div>
        </div>
        <nav aria-label="メインメニュー" className="sidebar-nav">
          {['WORKSPACE', 'GRAPH', 'MODEL'].map((group) => (
            <div className="nav-group" key={group}>
              <div className="nav-label">{group}</div>
              {pages.filter((page) => page.group === group).map((page) => (
                <button
                  className={`nav-button${activePage === page.id ? ' active' : ''}`}
                  key={page.id}
                  onClick={() => setActivePage(page.id)}
                  type="button"
                >
                  <span className="nav-icon" aria-hidden="true">{page.icon}</span>
                  <span>{page.label}</span>
                  {page.id === 'reviews' && reviewCandidateCount !== null ? <span className="nav-count">{reviewCandidateCount.toLocaleString()}</span> : null}
                </button>
              ))}
            </div>
          ))}
        </nav>
        <div className="sidebar-footer"><span className="status-dot" /> {project ? 'LOCAL / PROJECT READY' : 'LOCAL / NO PROJECT'}</div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="topbar-title">
            <div className="breadcrumb">プロジェクト / {project ? `${project.name}${dirty ? ' *' : ''}` : '未保存のプロジェクト'} / <span>{activePageDetails.label}</span></div>
            <h1>{activePageDetails.title}</h1>
          </div>
          <div className="top-actions">
            {!project ? <Chip>{isDesktopApp() ? 'LOCAL / OFFLINE' : 'DESKTOP APP REQUIRED'}</Chip> : <Chip>REV {project.revision}</Chip>}
            <button className="button" disabled={busy} onClick={() => void runAction(openProject)} type="button">開く</button>
            <button className="button" disabled={busy} onClick={() => void runAction(importExcel)} type="button">Excel取込</button>
            <button className="button" disabled={busy || !project} onClick={() => void runAction(saveProject)} type="button">保存</button>
            <button className="button" disabled={busy || !project} onClick={() => void runAction(exportMatrix)} type="button">結果CSV</button>
            <button className="button primary" disabled={busy || !project} onClick={() => void runAction(reevaluate)} type="button">↻ 再評価</button>
          </div>
        </header>

        {diagnostics.some((diagnostic) => diagnostic.severity === 'error') ? <div className="diagnostics-banner" role="alert">
          <strong>{diagnostics.filter((diagnostic) => diagnostic.severity === 'error').length}件の検証エラー</strong>
          <span>{diagnostics.find((diagnostic) => diagnostic.severity === 'error')?.message}</span>
          {diagnostics.find((diagnostic) => diagnostic.severity === 'error')?.source ? <small>{diagnostics.find((diagnostic) => diagnostic.severity === 'error')?.source?.file_name} / {diagnostics.find((diagnostic) => diagnostic.severity === 'error')?.source?.sheet}!{diagnostics.find((diagnostic) => diagnostic.severity === 'error')?.source?.column}{diagnostics.find((diagnostic) => diagnostic.severity === 'error')?.source?.row}</small> : null}
        </div> : null}

        <div className="page-body">{renderCurrentPage()}</div>
        {notice ? <div className="toast" role="status">{notice}</div> : null}
      </main>
    </div>
  );
}
