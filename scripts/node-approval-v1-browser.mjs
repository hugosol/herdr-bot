// Ticket 14 S3: read the actual published configurations and native Run facts.
// No synthetic execution, HTTP approval, component replacement or save action.
import fs from 'node:fs';
import { once } from 'node:events';
import { chromium, expect } from '../frontend/node_modules/@playwright/test/index.mjs';

const input = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const evidence = { passed: false, nodes: {}, http: [], cleanupErrors: [] };
let server, browser, child, exited;
try {
  server = await chromium.launchServer({ executablePath: input.chrome, headless: true });
  child = server.process();
  evidence.pid = child.pid;
  exited = once(child, 'exit');
  browser = await chromium.connect(server.wsEndpoint());
  evidence.browser = browser.version();
  const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
  page.on('request', request => evidence.http.push({ method: request.method(), url: request.url(), body: request.postData() }));
  for (const item of input.nodes) {
    const key = item.map + '/' + item.node;
    const actual = await (await page.request.get(input.base + '/api/scheduler/maps/' + item.map)).json();
    expect(actual).toEqual(item.definition);
    expect(actual.nodes[item.node].auto_approved).toBe(item.autoApproved);
    await page.goto(input.base + '?layout=expanded');
    await page.getByTestId('tab-scheduler').click();
    await page.locator(`[data-map-file="${item.map}"]`).click();
    await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: `${actual.nodes[item.node].title} ${item.node}`, exact: true }).click();
    await page.getByRole('button', { name: '修改 node', exact: true }).click();
    const form = page.getByRole('form', { name: 'node 编辑', exact: true });
    await expect(form).toBeVisible();
    await expect(form.getByRole('radio', { name: item.autoApproved ? 'auto' : 'manual', exact: true })).toBeChecked();
    const definitionText = await form.innerText();
    await page.screenshot({ path: `${input.output}/${key.replaceAll('/', '-')}-definition.png`, fullPage: true });
    await page.getByRole('button', { name: '放弃编辑', exact: true }).click();
    await page.goto(input.base + '/#workflow/' + item.runId);
    await page.getByRole('group', { name: 'Map 依赖图', exact: true }).getByRole('button', { name: `${actual.nodes[item.node].title} ${item.node}`, exact: true }).click();
    const facts = page.getByRole('region', { name: '当前执行事实', exact: true });
    const field = name => facts.locator('dt').filter({ hasText: new RegExp('^' + name + '$') }).locator('+ dd');
    await expect(field('id')).toContainText(item.executionId);
    await expect(field('autoApproved')).toHaveText(String(item.autoApproved));
    await expect(field('approved')).toHaveText('true · 已批准');
    await expect(field('approvalSource')).toHaveText(item.autoApproved ? 'auto · 自动批准（非用户 Yes）' : 'manual · 人工批准');
    const detail = await (await page.request.get(input.base + '/api/workflow/runs/' + item.runId)).json();
    const execution = detail.run.nodes[item.node].execution;
    expect(execution.id).toBe(item.executionId);
    expect(execution.autoApproved).toBe(item.autoApproved);
    expect(execution.approved).toBe(true);
    expect(execution.approvalSource).toBe(item.autoApproved ? 'auto' : 'manual');
    await page.screenshot({ path: `${input.output}/${key.replaceAll('/', '-')}-run.png`, fullPage: true });
    evidence.nodes[key] = { verified: true, definition: actual, public: detail, definitionText, factsText: await facts.innerText() };
  }
  expect(evidence.http.filter(request => !['GET', 'HEAD'].includes(request.method))).toEqual([]);
  evidence.passed = Object.keys(evidence.nodes).length === input.nodes.length && input.nodes.length > 0;
} catch (error) {
  evidence.error = error.stack || String(error);
} finally {
  if (browser) try { await browser.close(); } catch (error) { evidence.cleanupErrors.push(String(error)); }
  if (server) {
    try {
      await server.close();
      let timer;
      try { await Promise.race([exited, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('Owned Chrome did not exit')), 15000); })]); }
      finally { clearTimeout(timer); }
      evidence.processExited = child.exitCode !== null || child.signalCode !== null;
      if (!evidence.processExited) throw new Error('No owned Chrome exit confirmation');
    } catch (error) { evidence.cleanupErrors.push(String(error)); }
  }
  if (evidence.cleanupErrors.length || !evidence.processExited) evidence.passed = false;
  fs.writeFileSync(input.output + '/evidence.json', JSON.stringify(evidence, null, 2));
}
console.log(JSON.stringify({ passed: evidence.passed, evidence: input.output + '/evidence.json' }));
if (!evidence.passed) process.exitCode = 1;
