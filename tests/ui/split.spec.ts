import { test, expect, type Page } from "@playwright/test";
import { mkdir } from "node:fs/promises";

// UI / IPC contract only. The real import, grouping and output bytes are checked
// by src-tauri/src/split_tests.rs against files created inside a TempDir.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("gridflow_tour_completed", "true");
    const columns = [
      { key: "部门", name: "部门", dataType: "auto", suggestedType: "text" },
      { key: "金额", name: "金额", dataType: "auto", suggestedType: "number" },
    ];
    const rules = { version: 2, mergeMode: "bySheet", fileSelections: [], sources: [], columns: [], mappings: [], fixedCells: [], lookupColumns: [], outputSheets: [], mergeStrategy: "outer", removeEmptyRows: false, removeDuplicates: false, duplicateColumns: [], keepStrategy: "first", cleanNumeric: false, parseDates: false, includeSource: false, dateFormats: {}, emptyColumns: [] };
    const summary = { inputRows: 3, outputRows: 3, emptyRemoved: 0, duplicatesRemoved: 0, issueCount: 0, elapsedMs: 1 };
    const dataset = { id: "summary", name: "费用总表", createdAt: "2026-11-15", columns, summary, sample: false, rules, exports: [] };
    const sheets = ["费用明细", "部门明细"].map((name, i) => ({ id: `sheet${i}`, fileId: "input", name, rows: 4, width: 2, suggestedHeaderRow: 1, columns }));
    const source = { id: "input", name: "示例总表.xlsx", size: 3000, importedAt: "2026-11-15", storedPath: "D:\\synthetic\\copy.xlsx", encoding: null, sheets };
    const ws = { files: [], datasets: [dataset], tasks: [], schemes: [], rules, activeSheetId: null, settings: { passwordHash: null, lastExportDirectory: null }, dataDirectory: "D:\\synthetic\\workspace", locked: false, activities: [] };
    const cells = [[{ kind: "text", value: "销售部" }, { kind: "number", value: 10 }], [{ kind: "text", value: "销售部" }, { kind: "number", value: 20 }], [{ kind: "text", value: "采购部" }, { kind: "number", value: 30 }]];
    const calls: any[] = [];
    let plan: any = null, serial = 0;
    const invoke = async (command: string, args: any = {}) => {
      calls.push({ command, args: structuredClone(args) });
      if (command === "workspace") return structuredClone(ws);
      if (command === "save_draft" || command === "open_path" || command === "split_cancel") return null;
      if (command === "tasks") return [];
      if (command === "plugin:dialog|open") return args.options.directory ? "D:\\synthetic\\output" : "D:\\synthetic\\示例总表.xlsx";
      if (command.startsWith("plugin:event|")) return 1;
      if (command === "query_dataset") return { columns, rows: cells, rowNumbers: [1, 2, 3], total: 3, offset: args.query.offset, sample: false };
      if (command === "split_import") return structuredClone(source);
      if (command === "split_source_page") return { page: { columns, rows: [[{ kind: "text", value: "部门" }, { kind: "text", value: "金额" }], ...cells], rowNumbers: [1, 2, 3, 4], total: 4, offset: 0, sample: false }, appearance: { widths: [140, 140], columnWidths: {}, styles: [], cells: [], merges: [], rowHeights: {}, defaultRowHeight: 32, date1904: false, showGridLines: true, note: null } };
      if (command === "split_use_sheet") return { ...dataset, id: "imported", name: sheets.find(s => s.id === args.sheetId)!.name };
      if (command === "split_prepare") {
        if ((window as any).__SPLIT_FAIL_PREPARE_ONCE__) { (window as any).__SPLIT_FAIL_PREPARE_ONCE__ = false; throw { code: "DIRECTORY", message: "测试：保存位置暂时不可用" }; }
        await new Promise(resolve => setTimeout(resolve, 80));
        const groups = ["销售部", "采购部"].map((value, i) => {
          const id = JSON.stringify([1, value]);
          const edit = args.edits.find((e: any) => e.id === id);
          const pieces = args.request.naming.map((part: any) => part.kind === "value" ? value : part.kind === "text" ? part.text : part.kind === "source" ? args.request.sourceName : part.kind === "sheet" ? args.request.sheetName : part.kind === "date" ? args.request.date : "001");
          return { id, value, kind: 1, rows: i === 0 ? 2 : 1, selected: edit?.selected ?? true, filename: `${edit?.name ?? pieces.join(args.request.separator)}.${args.request.format}`, note: "", error: null, state: "ready", path: null };
        });
        plan = { id: `plan${++serial}`, groups, totalRows: 3, selectedRows: groups.filter(g => g.selected).reduce((n, g) => n + g.rows, 0), directory: args.output.directory ? `${args.output.directory}\\${args.output.folderName}` : null, note: null };
        return structuredClone(plan);
      }
      if (command === "split_page") {
        const rows = args.groupId ? cells.filter(row => JSON.stringify([1, row[0].value]) === args.groupId) : cells;
        return { columns, rows, rowNumbers: rows.map((_, i) => i + 1), total: rows.length, offset: args.offset, sample: false };
      }
      if (command === "split_execute") {
        if ((window as any).__SPLIT_FAIL_EXPORT_ONCE__) { (window as any).__SPLIT_FAIL_EXPORT_ONCE__ = false; throw { code: "EXPORT", message: "测试：目标文件夹暂时不可写" }; }
        plan.groups.forEach((g: any) => { if (g.selected) { g.state = "saved"; g.path = `${plan.directory}\\${g.filename}`; } }); return structuredClone(plan);
      }
      if (command === "split_progress") return { busy: false, phase: "", done: 0, total: 0, current: "" };
      return null;
    };
    (window as any).__SPLIT_CALLS__ = calls;
    (window as any).__TAURI_INTERNALS__ = { invoke, transformCallback: () => 1, unregisterCallback: () => {}, metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } } };
  });
});

async function enterSummary(page: Page) {
  await page.goto("/");
  await page.getByRole("navigation", { name: "工具入口" }).getByRole("button", { name: "表格拆分", exact: true }).click();
  await page.getByLabel("选择汇总表").selectOption("summary");
  await page.getByRole("button", { name: "使用此表，设置拆分" }).click();
  await page.getByLabel("拆分字段", { exact: true }).selectOption("部门");
  await expect(page.getByText("销售部.xlsx", { exact: true })).toBeVisible();
}

test("summary source supports live naming, per-file overrides, group details and export preview", async ({ page }) => {
  const errors: string[] = []; page.on("pageerror", e => errors.push(e.message));
  await enterSummary(page);
  await page.getByRole("button", { name: "固定文字", exact: true }).click();
  await page.getByLabel("固定文字 2", { exact: true }).fill("费用明细");
  await expect(page.getByText("销售部_费用明细.xlsx", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "销售部", exact: true }).click();
  await expect(page.getByText("销售部 · 2 行", { exact: true })).toBeVisible();
  await page.locator('.ag-header-cell[col-id="field:金额"] .ag-header-cell-label').click();
  await expect.poll(() => page.evaluate(() => (window as any).__SPLIT_CALLS__.filter((c: any) => c.command === "split_prepare").at(-1)?.args.request.query.sorts)).toEqual([{ key: "金额", descending: false }]);
  await expect(page.getByText("销售部 · 2 行", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "返回文件清单" }).click();
  await page.getByRole("button", { name: "修改 销售部 的文件名" }).click();
  await page.getByLabel("销售部 的自定义文件名", { exact: true }).fill("销售专用表");
  await expect(page.getByText("销售专用表.xlsx", { exact: true })).toBeVisible();
  await page.getByLabel("导出分类 采购部", { exact: true }).uncheck();
  await expect(page.getByRole("button", { name: "下一步：核对并导出（1）" })).toBeEnabled();
  await page.getByRole("button", { name: "下一步：核对并导出（1）" }).click();
  await expect(page.getByRole("button", { name: "导出所选文件（1）" })).toBeDisabled();
  await page.getByRole("button", { name: "选择保存文件夹", exact: true }).click();
  await expect(page.getByRole("button", { name: "导出所选文件（1）" })).toBeEnabled();
  await page.getByRole("button", { name: "导出所选文件（1）" }).click();
  await expect(page.getByRole("button", { name: "打开文件夹", exact: true })).toBeVisible();
  const executions = await page.evaluate(() => (window as any).__SPLIT_CALLS__.filter((c: any) => c.command === "split_execute"));
  expect(executions).toHaveLength(1); expect(executions[0].args.planId).toMatch(/^plan/);
  expect(errors).toEqual([]);
});

test("independent Excel import selects sheet and header without using summary import", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "开始表格拆分", exact: true }).click();
  await page.getByRole("button", { name: "导入 Excel", exact: true }).click();
  await page.getByRole("button", { name: "选择 Excel / CSV 文件", exact: true }).click();
  await page.getByLabel("选择工作表", { exact: true }).selectOption("sheet1");
  await expect(page.getByRole("grid", { name: "部门明细 Excel 预览" })).toBeVisible();
  await page.getByRole("button", { name: "使用此表，设置拆分" }).click();
  await page.getByLabel("拆分字段", { exact: true }).selectOption("部门");
  await expect(page.getByText("采购部.xlsx", { exact: true })).toBeVisible();
  const calls = await page.evaluate(() => (window as any).__SPLIT_CALLS__);
  expect(calls.some((c: any) => c.command === "import_files")).toBe(false);
  expect(calls.find((c: any) => c.command === "split_use_sheet").args).toEqual({ sheetId: "sheet1", range: { startRow: 1, endRow: 1, startColumn: 1, endColumn: 2 } });
  await page.getByRole("button", { name: "保存规则", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "规则已保存" })).toBeVisible();
  await page.getByRole("button", { name: "选择数据", exact: true }).click();
  await page.getByRole("spinbutton", { name: "表头起始行", exact: true }).fill("2");
  await expect(page.getByRole("button", { name: "设置拆分与命名", exact: true })).toBeDisabled();
  await expect(page.getByRole("button", { name: "核对并导出", exact: true })).toBeDisabled();
});

test("desktop layouts retain accessible actions and keep split settings across tool switching", async ({ page }) => {
  await enterSummary(page);
  await mkdir(".cache/split-ui", { recursive: true });
  for (const [width, height] of [[1366, 768], [1024, 640]]) {
    await page.setViewportSize({ width, height });
    await expect(page.getByRole("button", { name: "下一步：核对并导出（2）" })).toBeVisible();
    const bounds = await page.locator(".split-workspace").evaluate(el => ({ scroll: el.scrollWidth, client: el.clientWidth }));
    expect(bounds.scroll).toBeLessThanOrEqual(bounds.client + 1);
    await page.screenshot({ path: `.cache/split-ui/split-${width}.png` });
  }
  await page.getByRole("navigation", { name: "工具入口" }).getByRole("button", { name: "Excel 汇总", exact: true }).click();
  await page.getByRole("navigation", { name: "工具入口" }).getByRole("button", { name: "表格拆分", exact: true }).click();
  await expect(page.getByLabel("拆分字段", { exact: true })).toHaveValue("部门");
  await expect(page.getByText("销售部.xlsx", { exact: true })).toBeVisible();
});

test("stopping an automatic preview requires a fresh review before export", async ({ page }) => {
  await enterSummary(page);
  await page.getByLabel("文件名分隔符", { exact: true }).fill("-");
  await page.getByRole("button", { name: "停止", exact: true }).click();
  await expect(page.getByText("已停止核对，可点击“重新核对”继续。", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "下一步：核对并导出（2）" })).toBeDisabled();
  await page.getByRole("button", { name: "重新核对", exact: true }).click();
  await expect(page.getByRole("button", { name: "下一步：核对并导出（2）" })).toBeEnabled();
});

test("preview and export failures stay visible and can be retried without false success", async ({ page }) => {
  await enterSummary(page);
  await page.getByRole("button", { name: "下一步：核对并导出（2）" }).click();
  await page.evaluate(() => { (window as any).__SPLIT_FAIL_PREPARE_ONCE__ = true; });
  await page.getByRole("button", { name: "选择保存文件夹", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("保存位置暂时不可用");
  await expect(page.getByRole("button", { name: /^导出所选文件/ })).toBeDisabled();
  await page.getByRole("button", { name: "重新核对", exact: true }).click();
  await expect(page.getByRole("button", { name: "导出所选文件（2）" })).toBeEnabled();
  await page.evaluate(() => { (window as any).__SPLIT_FAIL_EXPORT_ONCE__ = true; });
  await page.getByRole("button", { name: "导出所选文件（2）" }).click();
  await expect(page.getByRole("alert")).toContainText("目标文件夹暂时不可写");
  await expect(page.getByRole("button", { name: "打开文件夹", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "导出所选文件（2）" }).click();
  await expect(page.getByRole("button", { name: "打开文件夹", exact: true })).toBeVisible();
});
