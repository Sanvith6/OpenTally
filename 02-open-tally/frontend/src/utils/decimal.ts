/**
 * Zero-float exact fixed-point financial arithmetic helper using native BigInt.
 * Guarantees zero IEEE 754 precision drift in TypeScript monetary calculations.
 */

const DEFAULT_SCALE = 4;
const SCALE_FACTORS: Record<number, bigint> = {
  0: 1n,
  1: 10n,
  2: 100n,
  3: 1000n,
  4: 10000n,
  6: 1000000n,
};

function getScaleFactor(scale: number): bigint {
  if (SCALE_FACTORS[scale]) return SCALE_FACTORS[scale];
  return 10n ** BigInt(scale);
}

/**
 * Parses a numeric string into a scaled BigInt.
 */
export function parseToScaledBigInt(value: string | number, scale: number = DEFAULT_SCALE): bigint {
  const str = String(value).trim();
  if (!str) return 0n;

  const isNeg = str.startsWith('-');
  const clean = isNeg || str.startsWith('+') ? str.slice(1) : str;

  const [intPart, fracPart = ''] = clean.split('.');
  const intBig = BigInt(intPart || '0');

  const paddedFrac = fracPart.slice(0, scale).padEnd(scale, '0');
  const fracBig = BigInt(paddedFrac);

  const factor = getScaleFactor(scale);
  const total = intBig * factor + fracBig;
  return isNeg ? -total : total;
}

/**
 * Formats a scaled BigInt back to a canonical decimal string.
 */
export function formatFromScaledBigInt(scaled: bigint, fromScale: number = DEFAULT_SCALE, displayScale: number = 2): string {
  const isNeg = scaled < 0n;
  const abs = isNeg ? -scaled : scaled;

  const factor = getScaleFactor(fromScale);
  const intPart = abs / factor;
  const fracPart = abs % factor;

  const fracStr = fracPart.toString().padStart(fromScale, '0');
  const truncatedFrac = fracStr.slice(0, displayScale).padEnd(displayScale, '0');

  const formatted = displayScale > 0 ? `${intPart}.${truncatedFrac}` : intPart.toString();
  return isNeg ? `-${formatted}` : formatted;
}

/**
 * Multiplies two decimal strings with exact midpoint-away-from-zero rounding to roundScale.
 */
export function multiplyDecimals(a: string, b: string, roundScale: number = 2): string {
  const scale = 4;
  const aBig = parseToScaledBigInt(a, scale);
  const bBig = parseToScaledBigInt(b, scale);

  // Result has scale = 8
  const rawProduct = aBig * bBig;

  // We want to reduce to roundScale
  const divisor = 10n ** BigInt(scale + scale - roundScale);
  const half = divisor / 2n;

  const isNeg = rawProduct < 0n;
  const abs = isNeg ? -rawProduct : rawProduct;
  const roundedAbs = (abs + half) / divisor;

  const result = isNeg ? -roundedAbs : roundedAbs;
  return formatFromScaledBigInt(result, roundScale, roundScale);
}

/**
 * Adds two decimal strings.
 */
export function addDecimals(a: string, b: string, displayScale: number = 2): string {
  const aBig = parseToScaledBigInt(a, displayScale);
  const bBig = parseToScaledBigInt(b, displayScale);
  return formatFromScaledBigInt(aBig + bBig, displayScale, displayScale);
}

/**
 * Subtracts two decimal strings (a - b).
 */
export function subtractDecimals(a: string, b: string, displayScale: number = 2): string {
  const aBig = parseToScaledBigInt(a, displayScale);
  const bBig = parseToScaledBigInt(b, displayScale);
  return formatFromScaledBigInt(aBig - bBig, displayScale, displayScale);
}

/**
 * Compares two decimal strings: -1 if a < b, 0 if a == b, 1 if a > b.
 */
export function compareDecimals(a: string, b: string): number {
  const aBig = parseToScaledBigInt(a, DEFAULT_SCALE);
  const bBig = parseToScaledBigInt(b, DEFAULT_SCALE);
  if (aBig < bBig) return -1;
  if (aBig > bBig) return 1;
  return 0;
}
