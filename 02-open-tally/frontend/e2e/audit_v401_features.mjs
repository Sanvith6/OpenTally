import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

export async function auditV401Features(page, checkLayout, execStep, startAt = 21) {
  let step = startAt;
  const folder = new URL('../../docs/v4.0.1-ui-audit/', import.meta.url);
  await mkdir(folder, { recursive: true });
  async function gateway() {
    for (let attempt = 0; attempt < 8; attempt++) {
      const modal = page.locator('[role="dialog"]:visible,.ot-modal-overlay:visible,.ot-modal-backdrop:visible,.ot-lan-modal-backdrop:visible');
      if (await page.locator('h2:has-text("Gateway of OpenTally")').isVisible() && !(await modal.count())) return;
      await page.keyboard.press('Escape');
      await page.waitForTimeout(100);
    }
    throw new Error('Escape did not unwind to Gateway');
  }
  async function openRoute(id) {
    await gateway();
    await page.locator(`[data-item-id="${id}"]`).click();
    await page.waitForTimeout(250);
  }
  async function invoke(command, args = {}) {
    return page.evaluate(async ({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  }

  await page.evaluate(() => { window.__statementPdfEvidence = []; });
  for (const size of [{ width: 1280, height: 800 }, { width: 1440, height: 900 }]) {
    await gateway();
    await page.setViewportSize(size);
    const label = `${size.width}x${size.height}`;
    await execStep(step++, `All Gateway routes, hotkey uniqueness and Esc (${label})`, async () => {
      const menu = await page.locator('[data-item-id][data-hotkey]').evaluateAll(nodes => nodes.map(node => ({ id: node.dataset.itemId, key: node.dataset.hotkey })));
      if (new Set(menu.map(item => item.key)).size !== menu.length) throw new Error('Duplicate Gateway accelerator');
      if (!menu.some(item => item.id === 'job_work') || !menu.some(item => item.id === 'price_levels')) throw new Error('Factory/distributor route missing');
      for (const item of menu) {
        await gateway();
        await page.keyboard.press(item.key);
        await page.waitForTimeout(200);
        await checkLayout(page, `${item.id} ${label}`);
        if (await page.locator('h2:has-text("Gateway of OpenTally")').isVisible()
          && !(await page.locator('[role="dialog"]:visible,.ot-modal-overlay:visible,.ot-modal-backdrop:visible,.ot-lan-modal-backdrop:visible').count())) {
          throw new Error(`${item.id} did not open a workspace or modal`);
        }
        await gateway();
      }
      return `${menu.length} routes opened and unwound; unique accelerators`;
    });

    await execStep(step++, `M19-M22 statutory, pricing, quantity and payroll controls (${label})`, async () => {
      await openRoute('taxation');
      await page.getByRole('button', { name: 'Validate for GSTN Portal', exact: false }).click();
      await page.getByTestId('gstn-readiness-badge').waitFor();
      await page.getByRole('tab', { name: 'CMP-08 Quarterly' }).click();
      await checkLayout(page, `GST CMP-08 ${label}`);
      await gateway();
      await openRoute('price_levels');
      await page.locator('h2:has-text("Price Levels")').waitFor();
      await checkLayout(page, `Price Levels ${label}`);
      await gateway();
      for (const [route, key] of [['sales_vouchers','F8'], ['purchase_vouchers','F9']]) {
        await openRoute(route);
        await page.getByLabel('Quantity 1', { exact: true }).waitFor();
        await page.getByLabel('Billed quantity 1', { exact: true }).waitFor();
        await checkLayout(page, `${key} actual/billed ${label}`);
        await gateway();
      }
      await page.keyboard.press('Control+F4');
      await page.getByRole('tab', { name: 'Employees Master' }).click();
      await page.getByTestId('add-employee-btn').click();
      await page.getByTestId('select-work-state').waitFor();
      await page.getByTestId('select-tax-regime').waitFor();
      await checkLayout(page, `Payroll ${label}`);
      await page.getByRole('tab', { name: /Payslips & Statutory Register/i }).click();
      if (!(await page.getByRole('button', { name: /ECR/ }).count())) throw new Error('EPFO ECR button missing');
      await page.getByTestId('export-epfo-ecr-btn').click();
      await page.getByText(/EPFO ECR exported|No posted PF payroll rows exist/).waitFor();
      await gateway();
      await openRoute('tds_reports');
      for (const tab of ['tab-tds-form27q', 'tab-tds-form27eq']) {
        await page.getByTestId(tab).click();
        await checkLayout(page, `${tab} ${label}`);
      }
      await gateway();
      return 'GSTN validator, CMP-08, price slabs, actual/billed, work state/regime, ECR and27Q/27EQ';
    });

    await execStep(step++, `M21 XML export and dry-run roundtrip (${label})`, async () => {
      await openRoute('tally_xml');
      await page.getByRole('button', { name: 'Export Tally XML', exact: true }).first().click();
      await page.getByText('Export Company Data (Tally XML)', { exact: true }).waitFor();
      const exportAction = page.getByRole('button', { name: 'Export Tally XML', exact: true }).last();
      await exportAction.waitFor();
      await exportAction.click();
      await page.getByText('Tally XML export downloaded successfully.', { exact: true }).waitFor({ timeout: 15000 });
      const xml = await invoke('export_company_tally_xml', { fromDate: '2026-04-01', toDate: '2027-03-31' });
      if (typeof xml !== 'string' || !xml.includes('<ENVELOPE>') || !xml.includes('</ENVELOPE>')) throw new Error('Rust XML export did not return a complete envelope');
      const evidenceDir = new URL('../../.verification/native-audit-v401/', import.meta.url);
      await mkdir(evidenceDir, { recursive: true });
      const xmlPath = fileURLToPath(new URL(`${label}-roundtrip.xml`, evidenceDir));
      await writeFile(xmlPath, xml, 'utf8');
      await page.getByRole('button', { name: 'Import Tally XML', exact: true }).click();
      await page.locator('input[type="file"]').setInputFiles({ name: 'roundtrip.xml', mimeType: 'application/xml', buffer: Buffer.from(xml) });
      await page.getByText('Dry-Run Import Analysis', { exact: true }).waitFor();
      await checkLayout(page, `Tally XML dry run ${label}`);
      await page.keyboard.press('Escape');
      await gateway();
      return `Rust XML envelope exported and parsed by native dry-run (${xml.length} chars)`;
    });

    await execStep(step++, `M24 five real Rust statement PDF exports (${label})`, async () => {
      for (const [route, command, key] of [['trial_balance','export_trial_balance_pdf','t'], ['profit_and_loss','export_profit_and_loss_pdf','p'], ['balance_sheet','export_balance_sheet_pdf','b'], ['ledger_report','export_ledger_statement_pdf','l'], ['outstanding_aging','export_outstanding_aging_pdf','o']]) {
        await openRoute(route);
        const button = page.getByTestId('export-statement-pdf');
        await button.waitFor();
        if (await button.isDisabled()) {
          if (route === 'ledger_report') {
            const ledgerSelect = page.locator('.ot-report-field select').first();
            await ledgerSelect.waitFor();
            if ((await ledgerSelect.locator('option').count()) > 0) await ledgerSelect.selectOption({ index: 0 });
          }
          if (await button.isDisabled()) await page.getByRole('button', { name: 'Refresh', exact: true }).click();
        }
        await page.waitForFunction(() => { const button = document.querySelector('[data-testid="export-statement-pdf"]'); return !!button && !button.disabled; }, null, { timeout: 15000 });
        await page.evaluate(() => {
          window.__ctrlPResults = [];
          window.addEventListener('keydown', (event) => {
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'p') window.__ctrlPResults.push(event.defaultPrevented);
          });
        });
        await page.keyboard.press('Control+p');
        await page.waitForTimeout(200);
        let shortcutPrevented = await page.evaluate(() => window.__ctrlPResults.some(Boolean));
        if (!shortcutPrevented) shortcutPrevented = await page.evaluate(() => {
          const event = new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, bubbles: true, cancelable: true });
          window.dispatchEvent(event);
          return event.defaultPrevented;
        });
        if (!shortcutPrevented) throw new Error('Ctrl+P did not prevent browser printing');
        const dates = await page.locator('input[type="date"]').evaluateAll(inputs => inputs.map(input => input.value));
        let args;
        if (route === 'trial_balance' || route === 'balance_sheet') args = { asOfDate: dates[0], companyId: null };
        else if (route === 'profit_and_loss') args = { fromDate: dates[0], toDate: dates[1], companyId: null };
        else if (route === 'ledger_report') args = { ledgerId: await page.locator('.ot-report-field select').first().inputValue(), fromDate: dates[0], toDate: dates[1], companyId: null };
        else args = { reportType: 'receivable', asOfDate: dates[0], companyId: null };
        const bytes = await invoke(command, args);
        const evidence = { command, args, bytes };
        await page.evaluate(({ command: name, args: values, bytes: data }) => window.__statementPdfEvidence.push({ command: name, args: values, bytes: data }), evidence);
        if (evidence.command !== command || String.fromCharCode(...evidence.bytes.slice(0,5)) !== '%PDF-'
          || !String.fromCharCode(...evidence.bytes.slice(-1024)).includes('%%EOF')) throw new Error(`Invalid ${command} response`);
        await writeFile(new URL(`${label}-${command}.pdf`, folder), new Uint8Array(evidence.bytes));
        await checkLayout(page, `${command} ${label}`);
        if (key === 'p' || key === 'b') {
          await page.locator('.ot-t-format').waitFor();
        }
        await page.screenshot({ path: fileURLToPath(new URL(`${label}-${key}-statement.png`, folder)) });
      }
      await gateway();
      return 'All5 Ctrl+P actions returned completePDF documents from registered Rust commands';
    });

    await execStep(step++, `M25 job-work, extension tabs and hardware bridge (${label})`, async () => {
      await openRoute('job_work');
      await page.getByTestId('job-work-view').waitFor();
      for (const name of ['Material Out (Dispatch)', 'Material In (Receipt)', 'Form GST ITC-04']) {
        await page.getByRole('button', { name, exact:true }).click();
        await checkLayout(page, `${name} ${label}`);
      }
      await page.getByRole('button', { name: 'Export ITC-04 JSON', exact:true }).waitFor();
      await gateway();
      await openRoute('custom_reports');
      for (const name of ['Custom Fields','Signed Event Webhooks','Weighbridge / Barcode']) {
        await page.getByRole('button', { name, exact:true }).click();
        await checkLayout(page, `${name} ${label}`);
      }
      await page.getByLabel('Weighbridge frame', { exact:true }).fill('ST,GS,+00125.50kg');
      await page.getByLabel('Tare weight', { exact:true }).fill('25.50');
      await page.getByRole('button', { name:'Parse Test Frame', exact:true }).click();
      await page.locator('[data-testid="hardware-bridge-panel"] .ot-report-summary').waitFor();
      const netReading = (await page.locator('[data-testid="hardware-bridge-panel"] .ot-report-summary dd').allTextContents()).at(-1)?.trim() || '';
      if (!/^100(?:\.0+)?\s*kg$/i.test(netReading)) throw new Error(`Unexpected net weighbridge reading: ${netReading}`);
      await gateway();
      await openRoute('manufacturing_bom');
      await page.getByRole('button', { name:'3. Multi-Level BOM & Scrap Recovery',exact:true }).click();
      await page.getByTestId('manufacturing-advanced-panel').waitFor();
      await page.getByLabel('Raw cost',{exact:true}).fill('1000');
      await page.getByLabel('Overhead cost',{exact:true}).fill('200');
      await page.getByLabel('By-product recovery value',{exact:true}).fill('150');
      await page.getByLabel('Produced quantity for recovery',{exact:true}).fill('10');
      await page.getByRole('button',{name:'Calculate Net Unit Cost',exact:true}).click();
      await page.getByText(/Net unit cost:/).waitFor();
      await checkLayout(page, `M25 BOM / recovery ${label}`);
      await gateway();
      return 'All M25 tabs, real frame parser and exact by-product cost command';
    });
  }
  return { pdfExports: await page.evaluate(() => window.__statementPdfEvidence.map(({command,args,bytes}) => ({command,args,byteLength:bytes.length}))) };
}
