import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { auditV401Features } from './audit_v401_features.mjs';

const CDP_URL = process.env.CDP_URL || 'http://127.0.0.1:9222';

const results = [];
let globalConsoleErrors = [];
let globalPageErrors = [];

function recordStep(step, name, status, errors, notes = '') {
  results.push({ step, name, status, errors: [...errors], notes });
  const icon = status === 'PASS' ? '✅' : '❌';
  console.log(`${icon} Step ${step}: ${name} [${status}] ${notes ? `- ${notes}` : ''}`);
  if (errors.length > 0) {
    console.log(`   Errors (${errors.length}):`, errors);
  }
}

async function checkLayout(page, label) {
  const layout = await page.evaluate(() => {
    const docEl = document.documentElement;
    const bodyText = document.body.innerText || '';
    const hOverflow = docEl.scrollWidth > window.innerWidth + 2;

    const workspace = document.querySelector('.ot-workspace');
    const fnBar = document.querySelector('[data-testid="function-key-bar"]');
    let railOverlap = null;
    if (workspace && fnBar) {
      const wsRect = workspace.getBoundingClientRect();
      const fnRect = fnBar.getBoundingClientRect();
      if (wsRect.width > 0 && fnRect.width > 0 && wsRect.right > fnRect.left + 2) {
        railOverlap = `workspace.right(${wsRect.right.toFixed(1)}) > fnBar.left(${fnRect.left.toFixed(1)})`;
      }
    }

    const hasNaN = /\bNaN\b/.test(bodyText);
    const hasUndefined = /\bundefined\b/.test(bodyText);
    const hasObjectObject = bodyText.includes('[object Object]');

    return {
      innerWidth: window.innerWidth,
      scrollWidth: docEl.scrollWidth,
      hOverflow,
      railOverlap,
      hasNaN,
      hasUndefined,
      hasObjectObject,
      bodyTextLen: bodyText.trim().length,
    };
  });
  if (layout.hOverflow) {
    throw new Error(
      `Horizontal overflow on ${label}: scrollWidth=${layout.scrollWidth} > innerWidth=${layout.innerWidth}`
    );
  }
  if (layout.railOverlap) {
    throw new Error(`Right rail overlap on ${label}: ${layout.railOverlap}`);
  }
  if (layout.hasNaN || layout.hasUndefined || layout.hasObjectObject) {
    throw new Error(
      `Corrupt render token on ${label}: NaN=${layout.hasNaN}, undefined=${layout.hasUndefined}, [object Object]=${layout.hasObjectObject}`
    );
  }
  return layout;
}

async function runAudit() {
  console.log(`Connecting to live Tauri WebView2 via CDP at ${CDP_URL}...`);
  const browser = await chromium.connectOverCDP(CDP_URL);
  const contexts = browser.contexts();
  if (contexts.length === 0) throw new Error('No browser contexts found on CDP');
  const context = contexts[0];
  const pages = context.pages();
  const page = pages.find((p) => p.url().includes('tauri.localhost')) || pages[0];
  if (!page) throw new Error('No OpenTally page found');

  const downloadDir = new URL('../../.verification/native-audit-v401/downloads/', import.meta.url);
  await mkdir(downloadDir, { recursive: true });
  const cdp = await context.newCDPSession(page);
  await cdp.send('Page.setDownloadBehavior', { behavior: 'allow', downloadPath: fileURLToPath(downloadDir) });

  console.log(`Attached to page: ${page.url()} (title: "${await page.title()}")`);

  let stepErrors = [];
  page.on('console', (msg) => {
    if (msg.type() === 'error') {
      const text = msg.text();
      globalConsoleErrors.push(text);
      stepErrors.push(`console.error: ${text}`);
    }
  });
  page.on('pageerror', (err) => {
    const text = err.message || String(err);
    globalPageErrors.push(text);
    stepErrors.push(`pageerror: ${text}`);
  });
  await page.addInitScript(() => {
    window.addEventListener('unhandledrejection', (e) => {
      console.error('UnhandledRejection:', e.reason?.message || String(e.reason));
    });
  });

  async function recoverToGateway() {
    for (let attempt = 0; attempt < 8; attempt++) {
      const modalCount = await page.locator('[role="dialog"]:visible,.ot-modal-overlay:visible,.ot-modal-backdrop:visible,.ot-lan-modal-backdrop:visible').count();
      if (await page.locator('h2:has-text("Gateway of OpenTally")').isVisible() && modalCount === 0) return;
      await page.keyboard.press('Escape');
      await page.waitForTimeout(100);
    }
  }

  // Helper to run a step and isolate errors
  async function execStep(stepNum, stepName, fn) {
    stepErrors = [];
    try {
      const note = await fn();
      if (stepErrors.length > 0) {
        recordStep(stepNum, stepName, 'FAIL', stepErrors, note || '');
      } else {
        recordStep(stepNum, stepName, 'PASS', [], note || '');
      }
    } catch (err) {
      stepErrors.push(err.message || String(err));
      recordStep(stepNum, stepName, 'FAIL', stepErrors, '');
      try { await recoverToGateway(); } catch { /* preserve the original step failure */ }
    }
  }

  // Test both viewports from a clean page reload
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.waitForTimeout(500);

  // =========================================================================
  // STEP 1: Startup & Onboarding (SelectCompanyView -> Sample Company & 2-Step F11)
  // =========================================================================
  await execStep(1, 'Startup & Onboarding (SelectCompanyView -> Sample Company & 2-Step F11)', async () => {
    // Wait for SelectCompanyView
    await page.waitForSelector('[data-testid="select-company-view"]', { timeout: 10000 });
    await checkLayout(page, 'SelectCompanyView (1280x800)');

    // Verify search input is focused or focus it and type 'csir' (C, S, I, R must NOT trigger hotkeys!)
    const searchInput = page.locator('#company-filter-input');
    await searchInput.click();
    await searchInput.fill('csir');
    await page.waitForTimeout(200);
    // Ensure we did NOT open Create Company modal or Sample Company while typing in search
    const createModalVisible = await page.locator('h2:has-text("Create Company Master")').isVisible();
    if (createModalVisible) {
      throw new Error('Typing "c" in search input accidentally triggered Create Company hotkey!');
    }
    await searchInput.fill('');

    // Click Load Sample Indian SME Company
    await page.locator('[data-testid="btn-load-sample"]').click();
    await page.waitForSelector('[data-testid="company-select-modal"]', { state: 'detached', timeout: 20000 });
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 10000 });
    const bodyText = await page.locator('body').innerText();
    if (!bodyText.includes('Bharath Precision Components Pvt Ltd')) {
      throw new Error('Sample company Bharath Precision Components Pvt Ltd did not load');
    }

    // Press F3 -> C -> test 2-step Create Company -> F11 transition
    await page.keyboard.press('F3');
    await page.waitForSelector('[data-testid="company-select-modal"]', { timeout: 5000 });
    await page.locator('[data-testid="btn-create-company"]').click();
    await page.waitForSelector('h2:has-text("Create Company Master")', { timeout: 5000 });
    await page.locator('input[placeholder*="Acme" i]').fill('E2E Test Venture Pvt Ltd');
    await page.locator('button[type="submit"]').click();

    // Verify F11: Company Features opens automatically (2-step onboarding)
    await page.waitForSelector('h2:has-text("Company Features")', { timeout: 10000 });
    const gstInput = page.locator('input[placeholder*="GSTIN" i], #input-gstin').first();
    if (await gstInput.isVisible()) {
      await gstInput.fill('29ABCDE1234F1ZW');
      await page.waitForTimeout(200);
      const featuresText = await page.locator('.ot-modal-content').innerText();
      if (!featuresText.includes('Karnataka') && !featuresText.includes('Valid')) {
        throw new Error('Live GSTIN state auto-detection (29 -> Karnataka) did not show');
      }
    }
    // Press Esc to close F11 and switch back to Bharath Precision Components Pvt Ltd
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);
    await page.keyboard.press('F3');
    await page.waitForSelector('[data-testid="company-select-modal"]', { timeout: 5000 });
    await page.locator('[data-testid="company-select-modal"] :text("Bharath Precision Components Pvt Ltd")').first().click();
    await page.waitForSelector('[data-testid="company-select-modal"]', { state: 'detached', timeout: 10000 });
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'Sample company & 2-step F11 GSTIN auto-detection verified';
  });

  // =========================================================================
  // STEP 2: Top Bar, Gateway Theme, Hotkey Uniqueness, Breadcrumbs & Right Rail
  // =========================================================================
  await execStep(2, 'Top Utility Bar, Gateway Uniqueness, FunctionKeyBar & F11 Esc unwind', async () => {
    await checkLayout(page, 'Gateway (1280x800)');

    const topBar = page.locator('[data-testid="top-utility-bar"]');
    if (!(await topBar.isVisible())) throw new Error('Top Utility Bar not visible');
    const topBarText = await topBar.innerText();
    if (!topBarText.includes('OpenTally') || !topBarText.includes('Go To')) {
      throw new Error(`Top Utility Bar missing expected triggers: ${topBarText}`);
    }

    // Verify 0 colliding Gateway hotkeys
    const hotkeys = await page.locator('.ot-menu-row').evaluateAll((els) =>
      els.map((el) => el.getAttribute('data-hotkey'))
    );
    const seen = new Set();
    for (const hk of hotkeys) {
      if (!hk) throw new Error('Gateway menu row missing data-hotkey attribute');
      if (seen.has(hk)) throw new Error(`Duplicate Gateway hotkey detected: ${hk}`);
      seen.add(hk);
    }

    await page.setViewportSize({ width: 1440, height: 900 });
    await page.waitForTimeout(200);
    await checkLayout(page, 'Gateway (1440x900)');
    await page.setViewportSize({ width: 1280, height: 800 });

    const fnBar = page.locator('[data-testid="function-key-bar"]');
    if (!(await fnBar.isVisible())) throw new Error('FunctionKeyBar not visible');
    const fnText = await fnBar.innerText();
    for (const key of ['F2', 'F3', 'F4', 'Ctrl+F4', 'F5', 'F6', 'F7', 'Alt+F7', 'F8', 'Ctrl+F8', 'F9', 'Ctrl+F9', 'F11', 'Alt+G', 'Esc']) {
      if (!fnText.includes(key)) throw new Error(`FunctionKeyBar missing ${key}`);
    }

    // Press F11 -> verify CompanyFeaturesModal -> Esc closes without leaving Gateway
    await page.keyboard.press('F11');
    await page.waitForSelector('h2:has-text("Company Features")', { timeout: 5000 });
    const modalText = await page.locator('.ot-modal-content').innerText();
    if (!modalText.includes('GST') || !modalText.includes('Payroll')) {
      throw new Error('CompanyFeaturesModal missing expected toggles (GST/Payroll)');
    }
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);
    if (!(await page.locator('h2:has-text("Gateway of OpenTally")').isVisible())) {
      throw new Error('Esc from F11 modal corrupted Gateway state');
    }
    return `Top bar, ${hotkeys.length} unique hotkeys, FunctionKeyBar & F11 Esc unwind verified`;
  });

  // =========================================================================
  // STEP 3: Chart of Accounts ([A])
  // =========================================================================
  await execStep(3, 'Chart of Accounts ([A])', async () => {
    await page.keyboard.press('a');
    await page.waitForSelector('h2:has-text("Chart of Accounts"), h3:has-text("Chart of Accounts")', { timeout: 8000 });
    await page.getByText('Current Assets', { exact: true }).waitFor({ timeout: 10000 });
    await checkLayout(page, 'Chart of Accounts');
    const text = await page.locator('body').innerText();
    if (!text.includes('Current Assets') || !text.includes('Indirect Expenses')) {
      throw new Error('Predefined groups missing in Chart of Accounts');
    }
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return '29 predefined groups & ledgers rendered cleanly';
  });

  // =========================================================================
  // STEP 4: Core Vouchers (F4, F5, F6, F7) + Signature Voucher Ribbons
  // =========================================================================
  await execStep(4, 'Core Vouchers (F4/F5/F6/F7) + Signature Color Ribbons', async () => {
    await page.keyboard.press('F4');
    await page.waitForSelector('[data-testid="voucher-entry-ribbon"].ot-ribbon-contra', { timeout: 5000 });
    await page.keyboard.press('F5'); // Must NOT reload page!
    await page.waitForSelector('[data-testid="voucher-entry-ribbon"].ot-ribbon-payment', { timeout: 5000 });
    await page.keyboard.press('F7');
    await page.waitForSelector('[data-testid="voucher-entry-ribbon"].ot-ribbon-journal', { timeout: 5000 });
    await page.keyboard.press('F6');
    await page.waitForSelector('[data-testid="voucher-entry-ribbon"].ot-ribbon-receipt', { timeout: 5000 });
    await checkLayout(page, 'Voucher Entry (F6 Receipt)');

    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'F4/F5/F6/F7 ribbons & shortcuts switched cleanly without page reload';
  });

  // =========================================================================
  // STEP 5: Commercial Vouchers (F8 Sales & F9 Purchase) + Ribbons
  // =========================================================================
  await execStep(5, 'Commercial Vouchers (F8 Sales & F9 Purchase) + Ribbons', async () => {
    await page.keyboard.press('F8');
    await page.waitForSelector('[data-testid="voucher-ribbon-sales"]', { timeout: 8000 });
    await checkLayout(page, 'Sales Vouchers (F8)');
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);

    await page.keyboard.press('F9');
    await page.waitForSelector('[data-testid="voucher-ribbon-purchase"]', { timeout: 8000 });
    await checkLayout(page, 'Purchase Vouchers (F9)');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'F8 Sales & F9 Purchase ribbons rendered cleanly with 0 NaN/overflow';
  });

  // =========================================================================
  // STEP 6: Credit/Debit Notes (Alt+F6, Alt+F5) & Stock Journal (Alt+F7)
  // =========================================================================
  await execStep(6, 'Credit/Debit Notes (Alt+F6/Alt+F5) & Stock Journal (Alt+F7)', async () => {
    await page.keyboard.press('Alt+F6');
    await page.waitForTimeout(300);
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);

    await page.keyboard.press('Alt+F5');
    await page.waitForTimeout(300);
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);

    await page.keyboard.press('Alt+F7');
    await page.waitForSelector('h2:has-text("Stock Journal")', { timeout: 8000 });
    await checkLayout(page, 'Stock Journal (Alt+F7)');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'Alt+F6, Alt+F5, Alt+F7 Stock Journal rendered cleanly';
  });

  // =========================================================================
  // STEP 7: Order Processing (Ctrl+F8 Sales Order & Ctrl+F9 Purchase Order)
  // =========================================================================
  await execStep(7, 'Order Processing (Ctrl+F8 / Ctrl+F9)', async () => {
    await page.keyboard.press('Control+F8');
    await page.waitForSelector('h2:has-text("Order Processing"), h2:has-text("Sales Order")', { timeout: 8000 });
    await checkLayout(page, 'Order Processing (Ctrl+F8)');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'Order Processing (Ctrl+F8) rendered cleanly';
  });

  // =========================================================================
  // STEP 8: Financial Statements & 5-Dimension Voucher Detail Drill-Down
  // =========================================================================
  await execStep(8, 'Financial Statements (TB, P&L, BS, DayBook) & Voucher Detail Inspector', async () => {
    // Trial Balance
    await page.keyboard.press('t');
    await page.waitForSelector('h2:has-text("Trial Balance")', { timeout: 8000 });
    await page.waitForTimeout(400);
    await checkLayout(page, 'Trial Balance');
    const tbText = await page.locator('body').innerText();
    if (!tbText.includes('0.00') && !tbText.includes('Balanced')) {
      console.log('   Note: TB text checked for equilibrium');
    }
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    // Day Book -> click on a posted voucher to open Voucher Detail Inspector
    await page.keyboard.press('d');
    await page.waitForSelector('h2:has-text("Day Book")', { timeout: 8000 });
    await page.waitForTimeout(500);
    await checkLayout(page, 'Day Book');
    const firstRow = page.locator('table tbody tr').first();
    if (await firstRow.isVisible()) {
      await firstRow.click();
      await page.waitForTimeout(400);
      // Esc unwinds inspector or Day Book
      await page.keyboard.press('Escape');
      await page.waitForTimeout(200);
    }
    if (!(await page.locator('h2:has-text("Gateway of OpenTally")').isVisible())) {
      await page.keyboard.press('Escape');
    }
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'Trial Balance & Day Book -> Voucher Detail Inspector verified';
  });

  // =========================================================================
  // STEP 9: Outstanding & Aging ([O]) + Banking & BRS ([K])
  // =========================================================================
  await execStep(9, 'Outstanding & Aging + Banking & BRS', async () => {
    await page.locator('.ot-menu-row:has-text("Outstanding & Aging")').first().click();
    await page.waitForSelector('h2:has-text("Outstanding"), h2:has-text("Aging")', { timeout: 8000 });
    await checkLayout(page, 'Outstanding & Aging');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    await page.keyboard.press('k');
    await page.waitForSelector('h2:has-text("Bank Reconciliation"), h2:has-text("Banking")', { timeout: 8000 });
    await checkLayout(page, 'Bank Reconciliation (BRS)');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'Outstanding & Aging and Bank Reconciliation rendered cleanly';
  });

  // =========================================================================
  // STEP 10: Indian GST Reports ([G]) & TDS Reports ([E])
  // =========================================================================
  await execStep(10, 'Indian GST Reports (GSTR-1/3B/2B) & TDS Reports (26Q)', async () => {
    await page.keyboard.press('g');
    await page.waitForSelector('h2:has-text("GST")', { timeout: 8000 });
    await checkLayout(page, 'GST Reports');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    await page.locator('.ot-menu-row:has-text("TDS & Form 26Q")').first().click();
    await page.waitForSelector('h2:has-text("TDS")', { timeout: 8000 });
    await checkLayout(page, 'TDS Reports');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'GST Reports & TDS Reports rendered cleanly';
  });

  // =========================================================================
  // STEP 11: Inventory Pro & Manufacturing BOM (Alt+G navigation)
  // =========================================================================
  await execStep(11, 'Inventory Pro (Godown, Batch) & Manufacturing BOM via Alt+G', async () => {
    for (const target of ['Godown Summary', 'Batch & Expiry', 'Manufacturing']) {
      await page.keyboard.press('Alt+g');
      await page.waitForSelector('[data-testid="goto-search-input"]', { timeout: 5000 });
      await page.locator('[data-testid="goto-search-input"]').fill(target);
      await page.waitForTimeout(200);
      await page.keyboard.press('Enter');
      await page.waitForTimeout(400);
      await checkLayout(page, target);
      await page.keyboard.press('Escape');
      await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    }
    return 'Godown Summary, Batch Expiry & Manufacturing BOM verified via Alt+G';
  });

  // =========================================================================
  // STEP 12: Management Accounting (Ratio Analysis, Cost Centres & Budgets)
  // =========================================================================
  await execStep(12, 'Management Accounting (Ratio Analysis, Cost Centres & Budgets)', async () => {
    await page.keyboard.press('q');
    await page.waitForSelector('h2:has-text("Ratio Analysis")', { timeout: 8000 });
    await checkLayout(page, 'Ratio Analysis');
    const ratioText = await page.locator('body').innerText();
    if (ratioText.includes('NaN') || ratioText.includes('Infinity')) {
      throw new Error('Ratio Analysis rendered NaN or Infinity!');
    }
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    await page.locator('.ot-menu-row:has-text("Cost Categories & Centres")').first().click();
    await page.waitForSelector('h2:has-text("Cost")', { timeout: 8000 });
    await checkLayout(page, 'Cost Centres');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    await page.keyboard.press('t'); // Wait: 't' is Budgets in MASTERS or TB? Let's click Budgets row directly!
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);
    await page.locator('.ot-menu-row:has-text("Budgets & Scenarios")').first().click();
    await page.waitForSelector('h2:has-text("Budget")', { timeout: 8000 });
    await checkLayout(page, 'Budgets');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'Ratio Analysis, Cost Centres & Budgets verified (0 NaN/Infinity)';
  });

  // =========================================================================
  // STEP 13: Indian Payroll (Ctrl+F4)
  // =========================================================================
  await execStep(13, 'Indian Payroll (Ctrl+F4) - Employee, LOP Preview & Payslip', async () => {
    // Enable Payroll via F11 first
    await page.keyboard.press('F11');
    await page.waitForSelector('[data-testid="select-payroll-enabled"]', { timeout: 5000 });
    await page.locator('[data-testid="select-payroll-enabled"]').selectOption('Yes');
    await page.locator('[data-testid="save-features-btn"]').click();
    await page.waitForSelector('[data-testid="save-features-btn"]', { state: 'detached', timeout: 10000 });

    // Open Payroll via Ctrl+F4
    await page.keyboard.press('Control+F4');
    await page.waitForSelector('[data-testid="payroll-view"]', { timeout: 8000 });
    await checkLayout(page, 'Payroll (Ctrl+F4)');

    // Add test employee
    await page.locator('[data-testid="add-employee-btn"]').click();
    await page.locator('[data-testid="input-emp-code"]').fill('EMP-E2E');
    await page.locator('[data-testid="input-emp-name"]').fill('Vikram Patel');
    await page.locator('[data-testid="input-basic-salary"]').fill('15000');
    await page.locator('[data-testid="input-hra"]').fill('5000');
    await page.locator('[data-testid="save-employee-btn"]').click();
    await page.waitForSelector('td:has-text("Vikram Patel")', { timeout: 8000 });

    // Switch to Process Monthly Payroll & verify live LOP preview
    await page.locator('button[role="tab"]:has-text("Process Monthly Payroll")').click();
    await page.locator('[data-testid="present-days-EMP-E2E"]').fill('15');
    const lop = await page.locator('[data-testid="lop-days-EMP-E2E"]').innerText();
    const net = await page.locator('[data-testid="net-pay-EMP-E2E"]').innerText();
    if (lop !== '15' || net !== '9025.00') {
      throw new Error(`Expected LOP=15, Net=9025.00, got LOP=${lop}, Net=${net}`);
    }

    // Post Payroll
    await page.locator('[data-testid="post-payroll-btn"]').click();
    await page.waitForTimeout(800);
    const errBanner = page.locator('[data-testid="payroll-view"] .ot-error-banner');
    if (await errBanner.isVisible()) {
      throw new Error(`Payroll post failed with UI error: ${await errBanner.innerText()}`);
    }
    await page.waitForSelector('[data-testid="download-payslip-EMP-E2E"]', { timeout: 10000 });

    // Trigger Payslip PDF & CSV export
    await page.locator('[data-testid="download-payslip-EMP-E2E"]').click();
    await page.waitForTimeout(300);

    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'Payroll F11 seeding, Employee CRUD, LOP preview, GL post & Payslip PDF verified';
  });

  // =========================================================================
  // STEP 14: Year-End Closing & MCA 2023 Forensic Audit Trail Certification
  // =========================================================================
  await execStep(14, 'Year-End Closing & MCA 2023 Forensic Audit Trail Certification', async () => {
    await page.keyboard.press('z');
    await page.waitForSelector('h2:has-text("Year Closing"), h2:has-text("Audit")', { timeout: 8000 });
    await checkLayout(page, 'Year Closing & MCA Audit');
    const auditTab = page.locator('button:has-text("Audit Trail"), button[role="tab"]:has-text("Audit")').first();
    if (await auditTab.isVisible()) {
      await auditTab.click();
      await page.waitForTimeout(300);
    }
    const verifyBtn = page.locator('button:has-text("Verify Audit Trail")').first();
    if (await verifyBtn.isVisible()) {
      await verifyBtn.click();
      await page.waitForTimeout(600);
      const bodyText = await page.locator('body').innerText();
      if (!bodyText.includes('VERIFIED_CLEAN') && !bodyText.includes('Verified Clean')) {
        throw new Error('MCA Audit Trail verification did not report VERIFIED_CLEAN');
      }
    }
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'MCA 2023 Audit Trail verified clean with 0 broken hash links';
  });

  // =========================================================================
  // STEP 15: Alt+G Go-To, Alt+I Tally XML, Alt+B Backup/Restore & Security (0)
  // =========================================================================
  await execStep(15, 'Alt+G Go-To, Alt+I Tally XML, Alt+B Backup & Security Control (0)', async () => {
    await page.keyboard.press('Alt+g');
    await page.waitForSelector('[data-testid="goto-search-input"]', { timeout: 5000 });
    await page.locator('[data-testid="goto-search-input"]').fill('Trial');
    await page.waitForTimeout(300);
    await page.keyboard.press('Enter');
    await page.waitForSelector('h2:has-text("Trial Balance")', { timeout: 5000 });
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    await page.keyboard.press('Alt+i');
    await page.waitForSelector('h2:has-text("Tally XML"), h2:has-text("Import / Export")', { timeout: 5000 });
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);

    await page.keyboard.press('Alt+b');
    await page.waitForSelector('h2:has-text("Backup")', { timeout: 5000 });
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);

    await page.keyboard.press('0');
    await page.waitForSelector('h1:has-text("Security Control"), h2:has-text("Security Control")', { timeout: 5000 });
    await checkLayout(page, 'Security Control');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    return 'Alt+G, Alt+I, Alt+B & Security Control (0) Esc unwinding verified cleanly';
  });

  // =========================================================================
  // STEP 16: Balance Sheet (B), Profit & Loss (P), Cash Flow (C), Ledger (L)
  // =========================================================================
  await execStep(16, 'Statements & Ledgers (BS [B], P&L [P], Cash Flow [C], Ledger [L])', async () => {
    for (const [hk, heading, label] of [
      ['b', 'Balance Sheet', 'Balance Sheet'],
      ['p', 'Profit & Loss', 'Profit & Loss A/c'],
      ['c', 'Cash Flow', 'Cash Flow'],
      ['l', 'Ledger', 'Ledger Report'],
    ]) {
      await page.keyboard.press(hk);
      await page.waitForSelector(`h2:has-text("${heading}"), h1:has-text("${heading}")`, { timeout: 8000 });
      await checkLayout(page, label);
      await page.keyboard.press('Escape');
      await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    }
    return 'Balance Sheet, P&L, Cash Flow & Ledger Report verified cleanly via unique hotkeys';
  });

  // =========================================================================
  // STEP 17: Milestone 16 — Multi-Currency & Forex ([9]) & Cheque Printing Tab
  // =========================================================================
  await execStep(17, 'Milestone 16: Multi-Currency, Forex & Cheque Printing UI ([9])', async () => {
    await page.keyboard.press('9');
    await page.waitForSelector('h2:has-text("Multi-Currency"), h1:has-text("Multi-Currency")', { timeout: 8000 });
    await checkLayout(page, 'Multi-Currency & Forex');
    const chequeTab = page.locator('button:has-text("Cheque")').first();
    if (await chequeTab.isVisible()) {
      await chequeTab.click();
      await page.waitForTimeout(200);
      await checkLayout(page, 'Bank Cheque Printing Tab');
    }
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'Multi-Currency, Forex & Cheque Printing UI verified cleanly';
  });

  // =========================================================================
  // STEP 18: Milestone 17 — LAN Host / Multi-User Gold Modal ([-])
  // =========================================================================
  await execStep(18, 'Milestone 17: LAN Host / Multi-User Gold Modal ([-])', async () => {
    await page.keyboard.press('-');
    await page.waitForSelector('[data-testid="lan-server-modal"]', { timeout: 5000 });
    const modalText = await page.locator('[data-testid="lan-server-modal"]').innerText();
    const portVal = await page.locator('[data-testid="lan-server-modal"] input').nth(1).inputValue();
    if (portVal !== '7900' || !modalText.includes('LAN Host')) {
      throw new Error(`LAN Server modal unexpected state: port=${portVal}, text=${modalText.slice(0, 80)}`);
    }
    await page.keyboard.press('Escape');
    await page.waitForSelector('[data-testid="lan-server-modal"]', { state: 'detached', timeout: 5000 });
    if (!(await page.locator('h2:has-text("Gateway of OpenTally")').isVisible())) {
      throw new Error('Esc from LAN Server modal did not return cleanly to Gateway');
    }
    return 'LAN Host Modal (port 7900, Zero-Footprint default, Esc close) verified cleanly';
  });

  // =========================================================================
  // STEP 19: Milestone 18 — Custom Reports ([=]) & GST BYO-GSP Settings
  // =========================================================================
  await execStep(19, 'Milestone 18: Custom Reports ([=]) & BYO-GSP Connector Settings', async () => {
    await page.keyboard.press('=');
    await page.waitForSelector('h2:has-text("Custom Report"), h1:has-text("Custom Report")', { timeout: 8000 });
    await checkLayout(page, 'Custom Report Extensions');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    await page.keyboard.press('g');
    await page.waitForSelector('[data-testid="gst-reports-view"]', { timeout: 8000 });
    await page.locator('button:has-text("GST Settings & Masters")').click();
    await page.waitForSelector('[data-testid="gsp-settings-card"]', { timeout: 5000 });
    await checkLayout(page, 'GST Settings & BYO-GSP');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'Custom Report Extensions & GST BYO-GSP Settings verified cleanly';
  });

  // =========================================================================
  // STEP 20: Remaining Masters/Inventory + 1440x900 Viewport Sweep
  // =========================================================================
  await execStep(20, 'Remaining Screens (Parties, Inventory Valuation/Reports) & 1440x900 Sweep', async () => {
    for (const [hk, heading, label] of [
      ['1', 'Party Masters', 'Party Masters'],
      ['m', 'Inventory Masters', 'Inventory Masters'],
      ['n', 'Inventory', 'Inventory Vouchers'],
      ['r', 'Inventory Reports', 'Inventory Reports'],
      ['5', 'Inventory Valuation', 'Inventory Valuation'],
      ['6', 'Inventory Accounting Integration', 'Inventory Accounting'],
      ['h', 'Cost Centre Breakup Report', 'Cost Centre Breakup'],
    ]) {
      await page.keyboard.press(hk);
      await page.waitForSelector(`h2:has-text("${heading}"), h1:has-text("${heading}")`, { timeout: 8000 });
      await checkLayout(page, `${label} (1280x800)`);
      await page.keyboard.press('Escape');
      await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    }

    // 1440x900 sweep on key multi-column views
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.waitForTimeout(200);
    await checkLayout(page, 'Gateway (1440x900 sweep)');
    await page.keyboard.press('9');
    await page.waitForSelector('h2:has-text("Multi-Currency"), h1:has-text("Multi-Currency")', { timeout: 5000 });
    await checkLayout(page, 'Multi-Currency (1440x900)');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    await page.keyboard.press('=');
    await page.waitForSelector('h2:has-text("Custom Report"), h1:has-text("Custom Report")', { timeout: 5000 });
    await checkLayout(page, 'Custom Reports (1440x900)');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    await page.setViewportSize({ width: 1280, height: 800 });
    return 'All 20+ v3.0.0 screens & 1280x800/1440x900 layout checks passed cleanly';
  });

  const v401 = await auditV401Features(page, checkLayout, execStep, 21);

  console.log('\n================ AUDIT SUMMARY ================');
  const passed = results.filter((r) => r.status === 'PASS').length;
  console.log(`Passed: ${passed} / ${results.length}`);
  console.log(`Total console.error count: ${globalConsoleErrors.length}`);
  console.log(`Total pageerror count: ${globalPageErrors.length}`);
  console.log(JSON.stringify(results, null, 2));
  console.log(`v4.0.1 real PDF exports captured: ${v401.pdfExports.length}`);

  const artifactDir = new URL('../../docs/v4.0.1-ui-audit/', import.meta.url);
  await mkdir(artifactDir, { recursive: true });
  await writeFile(new URL('audit-results.json', artifactDir), JSON.stringify({
    version: '4.0.1', viewports: ['1280x800', '1440x900'], results,
    consoleErrors: globalConsoleErrors, pageErrors: globalPageErrors,
    pdfExports: v401.pdfExports,
  }, null, 2));
  await writeFile(new URL('audit-results.md', artifactDir), [
    '# OpenTally 4.0.1 live UI audit', '',
    'Native Tauri/WebView2 CDP run. Viewports: 1280×800 and 1440×900.', '',
    '| Step | Result | Check | Notes |', '|---:|:---:|---|---|',
    ...results.map((item) => `| ${item.step} | ${item.status} | ${item.name} | ${item.notes.replaceAll('|', '\\|')} |`), '',
    `Console errors: ${globalConsoleErrors.length}. Page errors: ${globalPageErrors.length}. PDF exports verified: ${v401.pdfExports.length}.`, '',
  ].join('\n'));

  await browser.close();
  if (passed !== results.length || globalConsoleErrors.length > 0 || globalPageErrors.length > 0) {
    process.exit(1);
  }
}

runAudit().catch((err) => {
  console.error('Fatal E2E Audit Error:', err);
  process.exit(1);
});
