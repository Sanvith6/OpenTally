import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { InventoryTransactionsView } from '../InventoryTransactionsView';

const company = { id:'c1', name:'Stock Co', mailing_name:'Stock Co', country:'India', state:'Delhi', financial_year_begins:'2026-04-01', books_beginning_from:'2026-04-01', currency_symbol:'₹', currency_formal_name:'INR', currency_decimal_places:2, status:'active' as const, created_at:'', updated_at:'' };
describe('InventoryTransactionsView',()=>{
 it('renders all transaction forms and register',()=>{render(<InventoryTransactionsView activeCompany={company} onBack={()=>undefined}/>);expect(screen.getByText('Receipt Voucher')).toBeTruthy();for(const name of ['Delivery','Transfer','Adjustment','Stock Register']){fireEvent.click(screen.getByRole('button',{name}));expect(screen.getByRole('button',{name})).toBeTruthy();}});
 it('shows client validation before posting',()=>{render(<InventoryTransactionsView activeCompany={company} onBack={()=>undefined}/>);fireEvent.click(screen.getByRole('button',{name:'Save and Post'}));expect(screen.getByRole('alert').textContent).toContain('Select an item');});
});
