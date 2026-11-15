import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { App } from './App';

describe('STATEFLOW navigation shell', () => {
  it('switches between the six reference screens', () => {
    render(<App />);

    expect(screen.getByRole('heading', { level: 1, name: '統合状態遷移マトリクス' })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: /連動レビュー候補/ }));
    expect(screen.getByRole('heading', { level: 1, name: '連動レビュー候補一覧' })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: /レビュー抽出設定/ }));
    expect(screen.getByRole('heading', { level: 1, name: '連動レビュー候補の抽出設定' })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: /依存関係・実行順序/ }));
    expect(screen.getByRole('heading', { level: 1, name: '依存関係・実行順序エディタ' })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: '遷移ルール' }));
    expect(screen.getByRole('heading', { level: 1, name: '標準化した状態遷移ルール' })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: '実行設定' }));
    expect(screen.getByRole('heading', { level: 1, name: '機能の逐次実行設定' })).toBeTruthy();
  });
});
