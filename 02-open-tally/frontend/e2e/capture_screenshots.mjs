import { chromium } from 'playwright';
import fs from 'fs';
import path from 'path';

const CDP_URL = process.env.CDP_URL || 'http://127.0.0.1:9222';
const SCREENSHOT_DIR = path.resolve('assets/screenshots');

async function capture() {
  if (!fs.existsSync(SCREENSHOT_DIR)) {
    fs.mkdirSync(SCREENSHOT_DIR, { recursive: true });
  }

  console.log(`Connecting to live app via CDP at ${CDP_URL}...`);
  const browser = await chromium.connectOverCDP(CDP_URL);
  const context = browser.contexts()[0];
  const pages = context.pages();
  const page = pages.find((p) => p.url().includes('tauri.localhost')) || pages[0];
  if (!page) throw new Error('No page found');

  await page.setViewportSize({ width: 1440, height: 900 });
  await page.waitForTimeout(500);

  // Handle company selection
  const modal = page.locator('[data-testid="company-select-modal"]');
  if (await modal.isVisible()) {
    const bharath = modal.locator(':text("Bharath Precision Components Pvt Ltd")').first();
    if (await bharath.isVisible()) {
      await bharath.click();
    } else {
      const sampleBtn = modal.locator('button:has-text("Load Sample"), button:has-text("Sample Company")').first();
      if (await sampleBtn.isVisible()) {
        await sampleBtn.click();
      }
    }
    await page.waitForTimeout(800);
  } else {
    const view = page.locator('[data-testid="select-company-view"]');
    if (await view.isVisible()) {
      const bharath = view.locator(':text("Bharath Precision Components Pvt Ltd")').first();
      if (await bharath.isVisible()) {
        await bharath.click();
      } else {
        const sampleBtn = view.locator('button:has-text("Load Sample"), button:has-text("Sample Company")').first();
        if (await sampleBtn.isVisible()) {
          await sampleBtn.click();
        }
      }
      await page.waitForTimeout(800);
    }
  }

  // Ensure modals are closed to land on Gateway
  for (let i = 0; i < 3; i++) {
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);
  }
  await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 8000 });

  // 1. Gateway of OpenTally
  console.log('Capturing 01_gateway_terminal.png...');
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, '01_gateway_terminal.png') });

  // 2. Accounting Voucher Entry (F8 Sales Voucher with signature ribbon)
  console.log('Capturing 02_sales_voucher_ribbon.png...');
  await page.keyboard.press('F8');
  await page.waitForSelector('[data-testid="voucher-ribbon-sales"], h2:has-text("Sales")', { timeout: 8000 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, '02_sales_voucher_ribbon.png') });
  await page.keyboard.press('Escape');
  await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

  // 3. Financial Statements / Day Book & Voucher Detail Inspector
  console.log('Capturing 03_trial_balance.png...');
  await page.keyboard.press('t');
  await page.waitForSelector('h2:has-text("Trial Balance")', { timeout: 8000 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, '03_trial_balance.png') });
  await page.keyboard.press('Escape');
  await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

  // 4. Multi-Currency, Forex & Cheque Printing (M16)
  console.log('Capturing 04_multicurrency_cheque.png...');
  await page.keyboard.press('9');
  await page.waitForSelector('h2:has-text("Multi-Currency"), h1:has-text("Multi-Currency")', { timeout: 8000 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, '04_multicurrency_cheque.png') });
  await page.keyboard.press('Escape');
  await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

  // 5. Declarative Custom Reports & BYO-GSP Settings (M18)
  console.log('Capturing 05_custom_reports_builder.png...');
  await page.keyboard.press('=');
  await page.waitForSelector('h2:has-text("Custom Report"), h1:has-text("Custom Report")', { timeout: 8000 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, '05_custom_reports_builder.png') });
  await page.keyboard.press('Escape');
  await page.waitForSelector('h2:has-text("Gateway of OpenTally")', { timeout: 5000 });

  console.log('Screenshots successfully captured!');
  await browser.close();
}

capture().catch((err) => {
  console.error('Screenshot capture failed:', err);
  process.exit(1);
});
