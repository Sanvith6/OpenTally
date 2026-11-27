import { describe, expect, it } from 'vitest';
import { isPositiveFactoryDecimal, previewFactoryConsumption } from '../../../utils/factoryDecimal';

describe('Factory quantity previews preserve fixed-point arithmetic', () => {
  it('scales directly without rounding an intermediate ratio', () => {
    expect(previewFactoryConsumption('1', '3', '3')).toBe('1.0000');
    expect(previewFactoryConsumption('1.5', '3', '2')).toBe('2.2500');
    expect(previewFactoryConsumption('9999999999999999.99', '1', '1')).toBe('9999999999999999.9900');
  });
  it('accepts tiny positive quantities and rejects malformed decimal text', () => {
    expect(isPositiveFactoryDecimal('0.000001')).toBe(true);
    expect(isPositiveFactoryDecimal('Infinity')).toBe(false);
    expect(isPositiveFactoryDecimal('-0.001')).toBe(false);
    expect(isPositiveFactoryDecimal('0')).toBe(false);
    expect(previewFactoryConsumption('1', '1', '0')).toBe('—');
  });
});
