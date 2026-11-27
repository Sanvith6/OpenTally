import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { Gateway } from '../Gateway';

describe('factory and distributor Gateway navigation', () => {
  it('opens both new workspaces with unique keyboard accelerators', () => {
    const select = vi.fn();
    const { container } = render(<Gateway onSelectItem={select} />);
    for (const id of ['price_levels', 'job_work']) {
      const item = container.querySelector(`[data-item-id="${id}"]`);
      expect(item, `missing ${id} route`).not.toBeNull();
      const hotkey = item?.getAttribute('data-hotkey');
      expect(hotkey).toBeTruthy();
      fireEvent.keyDown(window, { key: hotkey });
      expect(select).toHaveBeenLastCalledWith(expect.objectContaining({ id }));
    }
    const keys = [...container.querySelectorAll('[data-hotkey]')].map(item => item.getAttribute('data-hotkey'));
    expect(new Set(keys).size).toBe(keys.length);
  });
});
