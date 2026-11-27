import { formatFromScaledBigInt, multiplyDecimals, parseToScaledBigInt } from '../../utils/decimal';

export function minorAmount(value: string): bigint {
  if (!/^[+-]?(?:\d+(?:\.\d{0,2})?|\.\d{1,2})$/.test(value.trim())) return 0n;
  return parseToScaledBigInt(value, 2);
}

interface TaxableLine { quantity: string; billed_quantity?: string | null; rate: string; discount_kind: string; discount_value: string }
export function taxableLinesTotal(lines: TaxableLine[]): string {
  const sum = lines.reduce((total, line) => {
    try {
      let amount = minorAmount(multiplyDecimals(line.billed_quantity ?? line.quantity ?? '0', line.rate || '0'));
      if (line.discount_kind === 'amount') amount -= minorAmount(line.discount_value || '0');
      else if (line.discount_kind === 'percentage') {
        const percent = parseToScaledBigInt(line.discount_value || '0', 4);
        amount -= (amount * percent + 500000n) / 1000000n;
      }
      return total + (amount > 0n ? amount : 0n);
    } catch { return total; }
  }, 0n);
  return formatFromScaledBigInt(sum, 2, 2);
}
