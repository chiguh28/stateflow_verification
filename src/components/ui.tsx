import type { ReactNode } from 'react';

export function PageHeading(props: {
  eyebrow: string;
  title: string;
  subtitle: string;
  actions?: ReactNode;
}) {
  return (
    <div className="section-heading">
      <div>
        <div className="eyebrow">{props.eyebrow}</div>
        <h2>{props.title}</h2>
        <p className="subtitle">{props.subtitle}</p>
      </div>
      {props.actions ? <div className="heading-actions">{props.actions}</div> : null}
    </div>
  );
}

export function PanelHeader(props: { title: string; trailing?: ReactNode }) {
  return (
    <div className="panel-header">
      <h3>{props.title}</h3>
      {props.trailing ? <div className="panel-trailing">{props.trailing}</div> : null}
    </div>
  );
}

export function MetricCard(props: {
  label: string;
  value: ReactNode;
  note: string;
  alert?: boolean;
}) {
  return (
    <div className={`metric-card${props.alert ? ' alert' : ''}`}>
      <span>{props.label}</span>
      <strong>{props.value}</strong>
      <small>{props.note}</small>
    </div>
  );
}

export function Chip(props: { children: ReactNode; className?: string }) {
  return <span className={`chip ${props.className ?? ''}`}>{props.children}</span>;
}

export function EmptyState(props: { title: string; detail: string }) {
  return (
    <div className="empty-state">
      <strong>{props.title}</strong>
      <p>{props.detail}</p>
    </div>
  );
}
