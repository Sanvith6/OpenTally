import { describe, expect, it } from 'vitest';
import { minorAmount, taxableLinesTotal } from '../vouchers/terminalAmounts';

describe('terminal exact minor units', () => {
  it('does not round-trip financial arithmetic through IEEE754', () => {
    expect(minorAmount('0.10') + minorAmount('0.20')).toBe(minorAmount('0.30'));
    expect(minorAmount('9007199254740993.01')).toBe(900719925474099301n);
  });
  it('computes quantities, fixed discounts and percentage discounts in integer units', () => {
    expect(taxableLinesTotal([
      { quantity: '3', rate: '0.10', discount_kind: 'none', discount_value: '0' },
      { quantity: '2', rate: '100.00', discount_kind: 'percentage', discount_value: '12.5' },
      { quantity: '1', rate: '50.00', discount_kind: 'amount', discount_value: '10.00' },
    ])).toBe('215.30');
  });
  it('does not treat incomplete field edits or nonfinite strings as valid money', () => {
    for (const value of ['-', '.', 'NaN', 'Infinity', '0.001']) expect(minorAmount(value)).toBe(0n);
  });
  it('uses billed quantity for money while preserving the actual movement quantity', () => {
    const line = { quantity: '12', billed_quantity: '10', rate: '20.00', discount_kind: 'none', discount_value: '0' };
    expect(taxableLinesTotal([line])).toBe('200.00');
    expect(line.quantity).toBe('12');
  });
});
