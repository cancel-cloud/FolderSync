// Browser layout regression using the real stylesheet, not native-app acceptance.
// Requires Playwright and Chrome; NODE_PATH may point to an existing installation.
const fs = require('node:fs');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true, channel: 'chrome' });
  try {
    for (const width of [800, 1100, 1920]) {
      const page = await browser.newPage({ viewport: { width, height: 812 } });
      const css = fs.readFileSync('src/style.css', 'utf8').replace('@import "tailwindcss";', '');
      await page.setContent(`<style>${css}</style><div id="root"><div class="app">
        <aside>FolderSync</aside><div class="workspace"><header>Tasks</header><main>
        <h1>Task details</h1><div class="table-wrap"><table style="min-width:2400px"><tr>
        <td>Relative path</td><td>Folder A</td><td>Folder B</td></tr></table></div>
        <div style="white-space:nowrap">${'Long diagnostic text '.repeat(70)}</div>
        <div style="height:1800px">Vertical content</div></main>
        <section class="run-status"><span class="run-idle">No active run</span></section>
        </div></div></div>`);
      const main = page.locator('main');
      await main.hover({ position: { x: 30, y: 220 } });
      await page.mouse.wheel(650, 0);
      await page.waitForTimeout(500);
      const positions = await page.evaluate(() => ({
        outer: document.scrollingElement.scrollLeft,
        page: document.querySelector('main').scrollLeft,
        workspaceRight: document.querySelector('.workspace').getBoundingClientRect().right,
      }));
      assert.equal(positions.outer, 0, `outer viewport moved sideways at ${width}px`);
      assert.equal(positions.page, 0, `page moved sideways at ${width}px`);
      assert.ok(positions.workspaceRight <= width, `workspace exceeds viewport at ${width}px`);
      await page.locator('.table-wrap').hover();
      await page.mouse.wheel(650, 0);
      await page.waitForTimeout(500);
      assert.ok(await page.locator('.table-wrap').evaluate(e => e.scrollLeft) > 0,
        'wide tables must remain horizontally accessible');
      await page.mouse.wheel(5000, 0);
      await page.waitForTimeout(500);
      assert.equal(await main.evaluate(e => e.scrollLeft), 0, 'table edge must not pan the page');
      // Use a fresh input surface so Chromium cannot latch the previous horizontal gesture.
      const vertical = await browser.newPage({ viewport: { width, height: 812 } });
      await vertical.setContent(await page.content());
      await vertical.locator('main').hover({ position: { x: 30, y: 220 } });
      await vertical.mouse.wheel(0, 400);
      await vertical.waitForTimeout(500);
      assert.ok(await vertical.locator('main').evaluate(e => e.scrollTop) > 0, 'vertical scrolling must work');
      await vertical.close();
      console.log(`PASS ${width}px: no page pan; vertical and nested table scrolling work`);
      await page.close();
    }
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
