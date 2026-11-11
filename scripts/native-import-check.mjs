// Real Windows/WebView2 import flow. Only the OS file picker is supplied by the test.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { copyFile, mkdir, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { chromium, expect } from "@playwright/test";

const source = path.resolve(process.argv[2] || "src-tauri/target/release/excel-workbench.exe");
const root = path.resolve(".cache", "native-import-" + Date.now());
const data = path.join(root, "data"), profile = path.join(root, "webview"), inputs = path.join(root, "inputs"), appDir = path.join(root, "app");
await Promise.all([data, profile, inputs, appDir].map(directory => mkdir(directory, { recursive: true })));
const app = path.join(appDir, "GridFlow.exe");
await copyFile(source, app);
await copyFile(path.resolve(".tools/duckdb/duckdb.dll"), path.join(appDir, "duckdb.dll"));
const paths = [];
for (let file = 0; file < 300; file++) {
  const filePath = path.join(inputs, `测试-${String(file).padStart(3, "0")}.csv`);
  const rows = ["编号," + Array.from({ length: 23 }, (_, column) => "数值" + (column + 1)).join(",")];
  for (let row = 0; row < 80; row++) rows.push(String(file * 80 + row).padStart(5, "0") + "," + Array(23).fill(row).join(","));
  await writeFile(filePath, rows.join("\n"));
  paths.push(filePath);
}
const portServer = net.createServer().listen(0, "127.0.0.1");
await once(portServer, "listening");
const port = portServer.address().port;
await new Promise(resolve => portServer.close(resolve));
let proc, browser, page;
const report = { source, root, files: 300, checks: [] };
async function ipc(command, args = {}) {
  return page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
}
try {
  proc = spawn(app, [], { cwd: appDir, windowsHide: true, stdio: "ignore", env: {
    ...process.env, GRIDFLOW_DATA_DIR: data, EXCEL_WORKBENCH_DATA_DIR: data, WEBVIEW2_USER_DATA_FOLDER: profile,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1 --disable-background-networking`,
  } });
  const endpoint = `http://127.0.0.1:${port}`;
  for (let retry = 0; retry < 150; retry++) {
    try { await fetch(endpoint + "/json/version", { signal: AbortSignal.timeout(500) }); break; } catch { await delay(100); }
  }
  browser = await chromium.connectOverCDP(endpoint, { timeout: 10000 });
  const context = browser.contexts()[0];
  page = context.pages()[0] || await context.waitForEvent("page");
  page.setDefaultTimeout(10000);
  await page.locator(".app-shell").waitFor();
  await page.evaluate(files => {
    localStorage.setItem("gridflow_tour_completed", "1");
    window.testImportFiles = files;
    const dialog = window.__TAURI_INTERNALS__.convertFileSrc("plugin:dialog|open", "ipc");
    const originalFetch = window.fetch.bind(window);
    window.fetch = (url, init) => url === dialog
      ? Promise.resolve(new Response(JSON.stringify(window.testImportFiles), { headers: { "Content-Type": "application/json", "Tauri-Response": "ok" } }))
      : originalFetch(url, init);
  }, paths);
  await delay(800); // Initial autosave has a 450 ms debounce.
  assert.equal(path.normalize((await ipc("workspace")).dataDirectory), path.normalize(data));
  await page.screenshot({ path: path.join(root, "before.png") });
  await page.getByRole("button", { name: "添加文件，开始汇总", exact: true }).click();
  const cancel = page.getByRole("button", { name: "取消导入", exact: true });
  await expect(cancel).toBeVisible({ timeout: 3000 });
  const progress = page.getByRole("progressbar", { name: "文件导入进度" });
  await expect(progress).toHaveAttribute("aria-valuemax", "300");
  await expect.poll(async () => Number(await progress.getAttribute("aria-valuenow")), { timeout: 15000 }).toBeGreaterThan(0);
  await page.screenshot({ path: path.join(root, "importing.png") });
  const ping = performance.now();
  await Promise.race([ipc("plugin:window|inner_size", { label: "main" }), delay(2000).then(() => { throw Error("native window stopped responding during import"); })]);
  report.windowResponseMs = Math.round(performance.now() - ping);
  await cancel.click();
  await expect(progress).toBeHidden({ timeout: 15000 });
  await expect(page.getByRole("status").filter({ hasText: "已停止导入" })).toBeVisible();
  const partial = await ipc("workspace");
  assert(partial.files.length > 0 && partial.files.length < 300, "cancel must retain only completed files");
  report.completedBeforeCancel = partial.files.length;
  report.checks.push("live progress, native window response, cancellation and completed files");

  const imported = new Set(partial.files.map(file => file.name));
  const remaining = paths.filter(file => !imported.has(path.basename(file)));
  await page.evaluate(files => { window.testImportFiles = files; }, remaining);
  const started = performance.now();
  await page.getByRole("button", { name: "添加文件", exact: true }).first().click();
  await expect(progress).toBeVisible();
  await expect(progress).toBeHidden({ timeout: 180000 });
  const complete = await ipc("workspace");
  assert.equal(complete.files.length, 300);
  assert(complete.files.every(file => file.sheets.length === 1 && file.sheets[0].rows === 81 && file.sheets[0].width === 24));
  report.resumeMs = Math.round(performance.now() - started);
  report.checks.push("all 300 files remain complete after cancellation and resume");
  let current;
  for (let attempt = 0; attempt < 60; attempt++) {
    current = await ipc("workspace");
    if (current.rules.fileSelections.filter(file => file.enabled).length === 300) break;
    await delay(50);
  }
  assert.equal(current.rules.fileSelections.filter(file => file.enabled).length, 300, "wait for the UI's saved selection, rather than reusing the import-completion snapshot");
  const rules = { ...current.rules, mergeMode: "single", includeSource: false };
  report.maxWindowResponseDuringSaveMs = 0;
  for (let index = 0; index < 20; index++) {
    const saving = ipc("save_scheme", { name: "window-response-" + index, rules });
    saving.catch(() => {});
    await delay(10);
    const ping = performance.now();
    await Promise.race([
      ipc("plugin:window|inner_size", { label: "main" }),
      delay(2000).then(() => { throw Error("saving a populated workspace blocked the native window for over 2s"); }),
    ]);
    report.maxWindowResponseDuringSaveMs = Math.max(report.maxWindowResponseDuringSaveMs, performance.now() - ping);
    await saving;
  }
  const task = await ipc("submit_task", { rules });
  let completedTask;
  for (let attempt = 0; attempt < 1200; attempt++) {
    const currentTask = (await ipc("tasks")).find(item => item.id === task.id);
    assert(!["failed", "cancelled", "interrupted"].includes(currentTask.state), JSON.stringify(currentTask));
    if (currentTask.state === "succeeded") { completedTask = currentTask; break; }
    await delay(50);
  }
  assert(completedTask, "full merge did not finish");
  const merged = (await ipc("workspace")).datasets.find(item => item.id === completedTask.datasetId);
  assert.equal(merged.summary.outputRows, 24000);
  assert.equal(merged.columns.length, 24);
  report.checks.push("native window remains responsive during 20 saved schemes and full 24000-row merge");
  await expect(page.locator(".excel-cell[data-cell='A2']")).toBeVisible();
  await page.screenshot({ path: path.join(root, "complete.png") });
  await page.setViewportSize({ width: 1024, height: 640 });
  await page.screenshot({ path: path.join(root, "compact.png") });
  report.status = "passed";
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  report.status = "failed";
  report.error = String(error.stack || error);
  if (page) await page.screenshot({ path: path.join(root, "failed.png"), timeout: 3000 }).catch(() => {});
  console.error(report.error);
  console.error("Evidence: " + root);
  process.exitCode = 1;
} finally {
  await writeFile(path.join(root, "report.json"), JSON.stringify(report, null, 2));
  await browser?.close().catch(() => {});
  if (proc?.exitCode === null) proc.kill();
}
