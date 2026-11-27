import { invoke } from '@tauri-apps/api/core';
import { VoucherWithLines } from '../types';

/** Posting remains an authenticated Rust transaction; no browser-side GL writes. */
export async function postAccountingVoucher(voucherId: string): Promise<VoucherWithLines> {
  return invoke<VoucherWithLines>('post_voucher', { req: { voucher_id: voucherId, actor_id: null, bank_allocations: null } });
}
