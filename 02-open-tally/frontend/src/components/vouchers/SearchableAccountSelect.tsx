import { useId, useState } from 'react';
import { advanceTerminalFocus } from './terminalKeyboard';

export interface AccountOption { id: string; name: string }
interface Props {
  id?: string;
  label: string;
  value: string;
  options: AccountOption[];
  onChange: (id: string) => void;
  order?: number;
}

/** Native keyboard semantics with an explicit, filtered and labelled listbox. */
export function SearchableAccountSelect({ id, label, value, options, onChange, order }: Props) {
  const generated = useId();
  const listId = `${id || generated}-options`;
  const [query, setQuery] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [selected, setSelected] = useState(0);
  const current = options.find((option) => option.id === value);
  const filtered = query === null ? options : options.filter((option) => option.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()));
  const commit = (option: AccountOption, input?: HTMLElement) => {
    onChange(option.id); setQuery(null); setOpen(false);
    if (input) advanceTerminalFocus(input);
  };
  return <div style={{ position: 'relative', minWidth: 0 }}>
    <input id={id} aria-label={label} role="combobox" aria-autocomplete="list" aria-expanded={open}
      aria-controls={listId} aria-activedescendant={open && filtered[selected] ? `${listId}-${selected}` : undefined}
      data-terminal-order={order} autoComplete="off" className="ot-input ot-ledger-select"
      value={query ?? current?.name ?? ''}
      onFocus={() => { setOpen(true); setSelected(0); }}
      onBlur={() => setOpen(false)}
      onChange={(event) => {
        const text = event.target.value;
        const exact = options.find((option) => option.id === text);
        if (exact) { onChange(exact.id); setQuery(null); setOpen(false); }
        else { if (value) onChange(''); setQuery(text); setSelected(0); setOpen(true); }
      }}
      onKeyDown={(event) => {
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          event.preventDefault(); event.stopPropagation(); setOpen(true);
          setSelected((index) => !open ? 0 : Math.max(0, Math.min(filtered.length - 1, index + (event.key === 'ArrowDown' ? 1 : -1))));
        } else if (event.key === 'Enter') {
          event.preventDefault(); event.stopPropagation();
          const option = filtered[selected];
          if (open && option) commit(option, event.currentTarget);
          else if (value) advanceTerminalFocus(event.currentTarget);
          else setOpen(true);
        } else if (event.key === 'Escape' && open) {
          event.preventDefault(); event.stopPropagation(); setOpen(false); setQuery(null);
        }
      }} />
    {open && <div id={listId} role="listbox" aria-label={`${label} choices`} style={{ position: 'absolute', zIndex: 50, top: '100%', left: 0, right: 0, maxHeight: 180, overflowY: 'auto', background: '#fff', border: '1px solid #d6e2dc', minWidth: 170 }}>
      {filtered.length ? filtered.map((option, index) => <div key={option.id} id={`${listId}-${index}`} role="option" aria-selected={selected === index}
        style={{ padding: '5px 8px', background: selected === index ? '#ffcf48' : '#fff', overflowWrap: 'anywhere' }}
        onMouseDown={(event) => event.preventDefault()} onClick={() => commit(option)}>{option.name}</div>) : <div style={{ padding: 8 }}>No matching accounts</div>}
    </div>}
  </div>;
}
