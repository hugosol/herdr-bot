// Ticket 14 S3: read the actual published configurations and native Run facts.
// No synthetic execution, HTTP approval, component replacement or save action.
import fs from 'node:fs';
import { expect } from '../frontend/node_modules/@playwright/test/index.mjs';
import { nativeVisibilityBrowser } from '../frontend/browser-tests/workflow/workflowVisibility.ts';

const input = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const evidence = { passed: false, nodes: {}, http: [], cleanupErrors: [] };
let owned;
try {
  owned = await nativeVisibilityBrowser(input.chrome);
  evidence.pid = owned.processId;
  evidence.profile = owned.profile;
  evidence.browser = owned.context.browser().version();
  const page = owned.page;
  await page.setViewportSize({ width: 1440, height: 1100 });
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
  if (owned) {
    try { await owned.cleanup(); }
    catch (error) { evidence.cleanupErrors.push(error.stack || String(error)); }
    evidence.processOwnership = owned.diagnostics();
    evidence.processExited = evidence.processOwnership.confirmedExit;
  }
  if (evidence.cleanupErrors.length || !evidence.processExited) evidence.passed = false;
  fs.writeFileSync(input.output + '/evidence.json', JSON.stringify(evidence, null, 2));
}
console.log(JSON.stringify({ passed: evidence.passed, evidence: input.output + '/evidence.json' }));
if (!evidence.passed) process.exitCode = 1;
