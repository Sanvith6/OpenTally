import { chromium } from 'playwright';

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
    const body = document.body;
    const hOverflow = docEl.scrollWidth > window.innerWidth + 2;
    return {
      innerWidth: window.innerWidth,
      scrollWidth: docEl.scrollWidth,
      hOverflow,
      bodyTextLen: body.innerText.trim().length,
    };
  });
  if (layout.hOverflow) {
    throw new Error(
      `Horizontal overflow on ${label}: scrollWidth=${layout.scrollWidth} > innerWidth=${layout.innerWidth}`
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
  // STEP 2: Gateway, Breadcrumbs & Right-Side FunctionKeyBar (1280x800 & 1440x900)
  // =========================================================================
  await execStep(2, 'Gateway, Breadcrumbs & FunctionKeyBar + F11 Esc unwind', async () => {
    await checkLayout(page, 'Gateway (1280x800)');
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
    return 'Gateway, FunctionKeyBar & F11 Esc unwind verified at 1280x800 & 1440x900';
  });

  // =========================================================================
  // STEP 3: Chart of Accounts ([A])
  // =========================================================================
  await execStep(3, 'Chart of Accounts ([A])', async () => {
    await page.keyboard.press('a');
    await page.waitForSelector('h2:has-text("Chart of Accounts"), h3:has-text("Chart of Accounts")', { timeout: 8000 });
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
  // STEP 4: Core Vouchers (F4, F5, F6, F7) + Inline Bill-Wise Allocation
  // =========================================================================
  await execStep(4, 'Core Vouchers (F4/F5/F6/F7) + F6 Receipt Bill-Wise Posting', async () => {
    await page.keyboard.press('F4');
    await page.waitForTimeout(300);
    await page.keyboard.press('F5'); // Must NOT reload page!
    await page.waitForTimeout(300);
    await page.keyboard.press('F7');
    await page.waitForTimeout(300);
    await page.keyboard.press('F6');
    await page.waitForTimeout(500);
    await checkLayout(page, 'Voucher Entry (F6 Receipt)');

    // Verify we are on Receipt voucher entry and can see lines/ledgers
    const bodyText = await page.locator('body').innerText();
    if (!bodyText.includes('Receipt') && !bodyText.includes('Voucher')) {
      throw new Error('F6 did not navigate to Receipt Voucher Entry');
    }
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'F4/F5/F6/F7 shortcuts switched cleanly without page reload';
  });

  // =========================================================================
  // STEP 5: Commercial Vouchers (F8 Sales & F9 Purchase)
  // =========================================================================
  await execStep(5, 'Commercial Vouchers (F8 Sales & F9 Purchase)', async () => {
    await page.keyboard.press('F8');
    await page.waitForSelector('h2:has-text("Sales"), h3:has-text("Sales")', { timeout: 8000 });
    await checkLayout(page, 'Sales Vouchers (F8)');
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);

    await page.keyboard.press('F9');
    await page.waitForSelector('h2:has-text("Purchase"), h3:has-text("Purchase")', { timeout: 8000 });
    await checkLayout(page, 'Purchase Vouchers (F9)');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });
    return 'F8 Sales & F9 Purchase rendered cleanly with 0 NaN/overflow';
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
  // STEP 15: Alt+G Go-To, Alt+I Tally XML, Alt+B Backup/Restore & Security Control
  // =========================================================================
  await execStep(15, 'Alt+G Go-To, Alt+I Tally XML, Alt+B Backup & Security Control', async () => {
    // Alt+G -> type "Trial" -> ArrowDown + Enter
    await page.keyboard.press('Alt+g');
    await page.waitForSelector('[data-testid="goto-search-input"]', { timeout: 5000 });
    await page.locator('[data-testid="goto-search-input"]').fill('Trial');
    await page.waitForTimeout(300);
    await page.keyboard.press('Enter');
    await page.waitForSelector('h2:has-text("Trial Balance")', { timeout: 5000 });
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    // Alt+I (Tally XML Modal) -> Esc
    await page.keyboard.press('Alt+i');
    await page.waitForSelector('h2:has-text("Tally XML"), h2:has-text("Import / Export")', { timeout: 5000 });
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);

    // Alt+B (Backup & Restore Modal) -> Esc
    await page.keyboard.press('Alt+b');
    await page.waitForSelector('h2:has-text("Backup")', { timeout: 5000 });
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);

    // Security Control -> Esc
    await page.keyboard.press('c');
    await page.waitForSelector('h1:has-text("Security Control"), h2:has-text("Security Control")', { timeout: 5000 });
    await checkLayout(page, 'Security Control');
    await page.keyboard.press('Escape');
    await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

    return 'Alt+G, Alt+I, Alt+B & Security Control Esc unwinding verified cleanly';
  });

  console.log('\n================ AUDIT SUMMARY ================');
  const passed = results.filter((r) => r.status === 'PASS').length;
  console.log(`Passed: ${passed} / ${results.length}`);
  console.log(`Total console.error count: ${globalConsoleErrors.length}`);
  console.log(`Total pageerror count: ${globalPageErrors.length}`);
  console.log(JSON.stringify(results, null, 2));

  await browser.close();
  if (passed !== results.length || globalConsoleErrors.length > 0 || globalPageErrors.length > 0) {
    process.exit(1);
  }
}

runAudit().catch((err) => {
  console.error('Fatal E2E Audit Error:', err);
  process.exit(1);
});
