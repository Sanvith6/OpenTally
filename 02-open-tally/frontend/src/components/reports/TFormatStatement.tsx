import './TFormatStatement.css';

export interface TFormatRow {
  label: string;
  amount?: string;
  group?: string;
  kind?: 'section' | 'detail' | 'subtotal' | 'balance';
}

interface Props {
  name: string;
  leftTitle: string;
  rightTitle: string;
  left: TFormatRow[];
  right: TFormatRow[];
  leftTotal: string;
  rightTotal: string;
}

export function TFormatStatement({ name, leftTitle, rightTitle, left, right, leftTotal, rightTotal }: Props) {
  const cell = (row: TFormatRow | undefined, side: string) => <>
    <td className={`ot-t-label ot-t-${row?.kind || 'detail'} ${side}`}>
      {row && <><span>{row.label}</span>{row.group && <small>{row.group}</small>}</>}
    </td>
    <td className={`money ot-t-${row?.kind || 'detail'} ${side}`}>{row?.amount || ''}</td>
  </>;
  return <table className="ot-report-table ot-t-format" aria-label={name}>
    <colgroup><col style={{ width: '34%' }} /><col style={{ width: '16%' }} /><col style={{ width: '34%' }} /><col style={{ width: '16%' }} /></colgroup>
    <thead><tr><th>{leftTitle}</th><th className="money">Amount (₹)</th><th>{rightTitle}</th><th className="money">Amount (₹)</th></tr></thead>
    <tbody>{Array.from({ length: Math.max(left.length, right.length) }, (_, index) =>
      <tr key={index}>{cell(left[index], 'ot-t-left')}{cell(right[index], 'ot-t-right')}</tr>)}</tbody>
    <tfoot><tr className="ot-t-grand-total"><th>Total</th><td className="money">{leftTotal}</td><th>Total</th><td className="money">{rightTotal}</td></tr></tfoot>
  </table>;
}
