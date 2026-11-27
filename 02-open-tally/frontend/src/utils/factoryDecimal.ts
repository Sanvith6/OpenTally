/** Factory previews use integer fixed-point arithmetic; Rust remains authoritative. */
const FACTORY_SCALE = 10n ** 28n;

function scaled(value: string): bigint | null {
  const clean = value.trim();
  if (!/^\+?\d+(?:\.\d{0,28})?$/.test(clean)) return null;
  const [integer, fraction = ''] = clean.replace(/^\+/, '').split('.');
  return BigInt(integer) * FACTORY_SCALE + BigInt(fraction.padEnd(28, '0'));
}

export function isPositiveFactoryDecimal(value: string): boolean {
  const parsed = scaled(value);
  return parsed !== null && parsed > 0n;
}

export function previewFactoryConsumption(quantity: string, component: string, base: string): string {
  const produced = scaled(quantity);
  const required = scaled(component);
  const baseQty = scaled(base);
  if (produced === null || required === null || baseQty === null || baseQty <= 0n) return '—';
  const denominator = baseQty * FACTORY_SCALE;
  const rounded = (produced * required * 10000n + denominator / 2n) / denominator;
  return `${rounded / 10000n}.${(rounded % 10000n).toString().padStart(4, '0')}`;
}
