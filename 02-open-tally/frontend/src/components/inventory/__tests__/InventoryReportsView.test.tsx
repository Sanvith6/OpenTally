import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { InventoryReportsView } from '../InventoryReportsView';

const company = { id:'c1', name:'Reports Co', mailing_name:'Reports Co', country:'India', state:'Delhi', financial_year_begins:'2026-04-01', books_beginning_from:'2026-04-01', currency_symbol:'₹', currency_formal_name:'INR', currency_decimal_places:2, status:'active' as const, created_at:'', updated_at:'' };
describe('InventoryReportsView',()=>{it('renders all five report tabs and controls',()=>{render(<InventoryReportsView activeCompany={company} onBack={()=>undefined}/>);for(const name of ['Summary','Register','Ledger','Valuation','Reconciliation'])expect(screen.getByRole('button',{name})).toBeTruthy();expect(screen.getByRole('button',{name:'Export CSV'})).toBeTruthy();expect(screen.getByLabelText('Report search')).toBeTruthy();});});
