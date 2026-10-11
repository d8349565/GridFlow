import { test, expect } from "@playwright/test";
import { writeFile, mkdir } from "node:fs/promises";

// These tests exercise the UI / IPC contract only. Calculation and file I/O are
// covered by the real Rust flows suite; no mock or sample engine ships in the app.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    try { localStorage.setItem("gridflow_tour_completed", "true"); } catch {}
    window.addEventListener("DOMContentLoaded", () => {
      try { localStorage.setItem("gridflow_tour_completed", "true"); } catch {}
    });
    const text = (value: string) => ({ kind: "text", value });
    const number = (value: number) => ({ kind: "number", value });
    const columns = [
      { key: "门店", name: "门店", dataType: "auto", suggestedType: "text" },
      { key: "金额", name: "金额", dataType: "auto", suggestedType: "number" },
      {
        key: "__source",
        name: "数据源",
        dataType: "text",
        suggestedType: "text",
      },
    ];
    const rules = {
      version: 2,
      sources: [],
      columns: [],
      mappings: [],
      fixedCells: [],
      mergeStrategy: "outer",
      removeEmptyRows: false,
      removeDuplicates: false,
      duplicateColumns: [],
      keepStrategy: "first",
      cleanNumeric: false,
      parseDates: false,
      includeSource: true,
      dateFormats: {},
      emptyColumns: [],
    };
    const ws: any = {
      files: [],
      datasets: [],
      tasks: [],
      schemes: [],
      rules,
      activeSheetId: null,
      settings: { passwordHash: null, lastExportDirectory: null },
      dataDirectory: "D:\\Excel工作台\\data",
      locked: false,
    };
    const calls: any[] = [];
    (window as any).__TEST_CALLS__ = calls;
    let callbacks = 1;
    let taskIndex = 0;
    const invoke = async (command: string, args: any = {}) => {
      calls.push({ command, args: structuredClone(args) });
      if (command === "plugin:dialog|open")
        return ["D:\\输入\\一月.xlsx", "D:\\输入\\二月.xlsx"];
      if (command === "plugin:dialog|save") return "D:\\导出\\汇总.xlsx";
      if (command === "workspace") { await new Promise(resolve => setTimeout(resolve, 25)); return structuredClone(ws); }
      if (command === "save_draft") {
        ws.rules = structuredClone(args.rules);
        ws.activeSheetId = args.activeSheetId;
        return null;
      }
      if (command === "import_files" || command === "load_sample_files") {
        ws.files = Array.from({ length: (window as any).__TEST_MANY_FILES__ ? 32 : 2 }, (_, index) => index + 1).map((i) => ({
          id: `f${i}`,
          name: `${i}月.xlsx`,
          size: 2048,
          importedAt: new Date().toISOString(),
          storedPath: `D:\\data\\${i}.xlsx`,
          encoding: null,
          sheets: [
            {
              id: `s${i}`,
              fileId: `f${i}`,
              name: "销售",
              rows: 4,
              width: 2,
              suggestedHeaderRow: 1,
              columns: columns.slice(0, 2),
            },
          ],
        }));
        if ((window as any).__TEST_MULTI_SHEET__) ws.files[1].sheets.push({ ...ws.files[1].sheets[0], id: "s3", name: "部门明细" });
        if ((window as any).__TEST_COMMON_PREFIX__) ws.files.forEach((file: any, index: number) => { file.name = `销售_BJ0${index + 1}_北京.xlsx`; });
        ws.rules = {
          ...rules,
          sources: ws.files.flatMap((f: any) => f.sheets.map((sheet: any) => ({
            sheetId: sheet.id,
            enabled: true,
            headerRow: 1,
            sourceName: f.name,
          }))),
        };
        ws.activeSheetId = "s1";
        return ws.files.map((f: any) => ({
          path: f.name,
          file: f,
          error: null,
        }));
      }
      if (command === "preview_sheet")
        return {
          columns: [
            { ...columns[0], key: "c0", name: "A" },
            { ...columns[1], key: "c1", name: "B" },
          ],
          rows: [
            [text("门店"), text("金额")],
            [text("浦东"), number(1200)],
            [text("虹桥"), number(800)],
            [text("浦东"), number(500)],
          ].slice(args.offset, args.offset + args.limit),
          rowNumbers: [1, 2, 3, 4].slice(args.offset, args.offset + args.limit),
          total: 4,
          offset: args.offset,
          sample: false,
        };
      if (command === "preview_appearance") return { widths:[150,130],rowHeights:{},styles:[{ bold:false,italic:false,fontSize:11,color:"#263449",background:"",align:"",wrap:false,numberFormat:"General",borders:[] }],cells:[],merges:(window as any).__TEST_MERGED__ ? [{startRow:1,endRow:2,startColumn:1,endColumn:1}] : [],note:null };
      if (command === "input_columns") return columns.slice(0, 2);
      if (command === "suggestions")
        return columns
          .slice(0, 2)
          .map((c) => ({
            ...c,
            examples: [
              {
                before: c.key === "金额" ? ((window as any).__TEST_CURRENCY_HINT__ ? text("¥1,200") : number(1200)) : text("浦东"),
                after: c.key === "金额" ? number(1200) : text("浦东"),
                error: null,
              },
            ],
          }));
      if (command === "available_columns")
        return [
          ...columns,
          ...args.rules.fixedCells.map((r: any) => ({
            key: `fixed:${r.id}`,
            name: r.name,
            dataType: "auto",
            suggestedType: "text",
          })),
        ];
      if (command === "submit_task") {
        taskIndex++;
        const ds = {
          id: `result${taskIndex}`,
          name: "月度汇总",
          createdAt: new Date().toISOString(),
          columns,
          summary: {
            inputRows: 6,
            outputRows: 6,
            emptyRemoved: 0,
            duplicatesRemoved: 0,
            issueCount: 0,
            elapsedMs: 25,
          },
          sample: false,
          rules: args.rules,
          exports: [],
        };
        ws.datasets = [ds,...ws.datasets];
        const task = {
          id: `task${taskIndex}`,
          state: "pending",
          phase: "等待处理",
          progress: 0,
          createdAt: new Date().toISOString(),
          finishedAt: null,
          error: null,
          datasetId: null,
          rules: args.rules,
        };
        ws.tasks = [task];
        return task;
      }
      if (command === "tasks") {
        if (ws.tasks.length) {
          ws.tasks[0] = {
            ...ws.tasks[0],
            state: "succeeded",
            phase: "处理完成",
            progress: 100,
            datasetId: ws.datasets[0].id,
          };
        }
        return structuredClone(ws.tasks);
      }
      if (command === "query_dataset")
        return {
          columns,
          rows: [
            [text("浦东"), number(1200), text("一月.xlsx")],
            [text("虹桥"), number(800), text("二月.xlsx")],
          ],
          rowNumbers: [1, 2],
          total: 2,
          offset: args.query.offset,
          sample: false,
        };
      if (command === "distinct_values") return ["浦东", "虹桥"];
      if (command === "save_scheme") {
        const scheme = { id: "saved1", name: args.name || "销售汇总设置", rules: structuredClone(args.rules), sheets: [], updatedAt: new Date().toISOString() };
        ws.schemes = [scheme];
        return scheme;
      }
      if (command === "apply_scheme")
        return { ...structuredClone(ws.schemes.find((s: any) => s.id === args.schemeId).rules), sources: args.rules.sources };
      if (command === "aggregate")
        return {
          columns: [
            columns[0],
            { ...columns[1], key: "value:1:0", dataType: "number" },
          ],
          rows: [
            [text("浦东"), number(1700)],
            [text("虹桥"), number(800)],
            [text("总计"), number(2500)],
          ],
          totalRecords: 6,
          groupCount: 2,
          truncated: false,
          spec: args.spec,
        };
      if (command === "delete_items") {
        if (args.kind === "files") {
          ws.files = ws.files.filter((f: any) => !args.ids.includes(f.id));
          ws.rules.sources = ws.rules.sources.filter((s: any) => !args.ids.some((id: string) => s.sheetId.startsWith(id.replace("f", "s"))));
        }
        return null;
      }
      if (command === "disk_usage") return 4096;
      if (command.startsWith("plugin:event|")) return 1;
      if (command === "export_dataset" || command === "export_analysis")
        return args.path;
      return null;
    };
    (window as any).__TAURI_INTERNALS__ = {
      invoke,
      transformCallback: () => callbacks++,
      unregisterCallback: () => {},
      metadata: {
        currentWindow: { label: "main" },
        currentWebview: { label: "main" },
      },
    };
  });
});
test("layout space for original Excel", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "添加文件", exact: true }).click();
  await expect(page.locator('.excel-cell[data-cell="A1"]')).toBeVisible();
  const sourceDensity = { rowHeight: await page.locator(".source-file-row").first().evaluate((el) => el.getBoundingClientRect().height), listHeight: await page.locator(".file-scroll").evaluate((el) => el.getBoundingClientRect().height) };
  expect(sourceDensity.rowHeight).toBeLessThanOrEqual(38);
  await mkdir(".cache", { recursive: true });
  await writeFile(".cache/source-files-after.json",JSON.stringify(sourceDensity,null,2));
  const metrics = await page.evaluate(() => {
    const rect = (selector: string) => {
      const area = document.querySelector(selector)!.getBoundingClientRect();
      return { x: area.x, y: area.y, width: area.width, height: area.height };
    };
    return { viewport: { width: innerWidth, height: innerHeight }, excel: rect(".excel-scroll"), files: rect(".file-sidebar"), configuration: rect(".rules-panel") };
  });
  expect(metrics.excel.height).toBeGreaterThanOrEqual(560);
  expect(metrics.excel.width).toBeGreaterThanOrEqual(870);
  await writeFile(".cache/layout-after.json", JSON.stringify(metrics, null, 2));
  await page.screenshot({ path: ".cache/layout-after.png" });
  await page.locator('.excel-cell[data-cell="B3"]').click();
  const cellStyle = await page.locator('.excel-cell[data-cell="B3"]').evaluate((cell) => ({ font: getComputedStyle(cell).fontSize, width: cell.getBoundingClientRect().width }));
  await page.getByRole("button", { name: "收起文件列表", exact: true }).click();
  await page.getByRole("button", { name: "专注表格", exact: true }).click();
  await expect(page.locator(".file-sidebar, .rules-panel")).toHaveCount(0);
  expect(await page.locator(".excel-scroll").evaluate((el) => el.getBoundingClientRect().width)).toBe(1366);
  await expect(page.locator(".excel-address")).toHaveText("B3");
  expect(await page.locator('.excel-cell[data-cell="B3"]').evaluate((cell) => ({ font: getComputedStyle(cell).fontSize, width: cell.getBoundingClientRect().width }))).toEqual(cellStyle);
  await page.screenshot({ path: ".cache/layout-focus.png" });
  await page.getByRole("button", { name: "退出专注", exact: true }).click();
  await expect(page.locator(".file-sidebar")).toHaveCount(0);
  await expect(page.locator(".rules-panel")).toBeVisible();
  await page.getByRole("button", { name: "显示文件列表", exact: true }).click();
  await page.setViewportSize({ width: 1024, height: 640 });
  await expect(page.getByRole("button", { name: "设为表头", exact: true })).toBeVisible();
  const small = await page.evaluate(() => ({ width: document.documentElement.scrollWidth, height: document.documentElement.scrollHeight, excelHeight: document.querySelector(".excel-scroll")!.getBoundingClientRect().height, toolbarWidth: document.querySelector(".configuration-toolbar")!.scrollWidth, paneWidth: document.querySelector(".main-pane")!.clientWidth }));
  expect(small.width).toBe(1024);
  expect(small.height).toBe(640);
  expect(small.excelHeight).toBeGreaterThanOrEqual(440);
  expect(small.toolbarWidth).toBeLessThanOrEqual(small.paneWidth);
  await page.screenshot({ path: ".cache/layout-small.png" });
});
test("click configuration, generation, filter and export without typing", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await expect(page.getByText("把多份表格，")).toBeVisible();
  await page.getByRole("button", { name: "添加文件", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByText("销售", { exact: true }).first()).toBeVisible();
  await page.getByRole("button", { name: "选择第 2 行", exact: true }).click();
  await page.getByRole("button", { name: "设为表头", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("从第 3 行开始读取");
  await page.getByRole("button", { name: "使用这片表头", exact: true }).click();
  await page.locator('.excel-cell[data-cell="B3"]').click();
  await page.getByRole("button", { name: "添加为新列", exact: true }).click();
  await expect(page.getByText("销售 ! B3")).toBeVisible();
  await page.getByRole("button", { name: "加入汇总表" }).click();
  await page.evaluate(() => { (window as any).__TEST_CURRENCY_HINT__ = true; });
  await page.getByRole("button", { name: "格式建议" }).click();
  await page.getByLabel("调整 金额").check();
  await page.getByRole("button", { name: "仅使用调整" }).click();
  await page.screenshot({ path: ".cache/staged-configuration.png" });
  await page
    .getByRole("button", { name: "预览结果", exact: true })
    .first()
    .click();
  await expect(page.locator(".rules-panel")).toHaveCount(0);
  await expect(page.locator(".file-sidebar")).toHaveCount(0);
  await expect(page.locator(".pane-tabs button.active")).toContainText(
    "汇总表",
    { timeout: 12000 },
  );
  await page.getByLabel("选择要筛选的列").selectOption("门店");
  await page.getByLabel("浦东", { exact: true }).check();
  await page.getByRole("button", { name: "应用筛选" }).click();
  await page.getByRole("button", { name: "保存筛选后的表格" }).click();
  await expect(page.locator('.ag-cell[col-id="field:门店"]').first()).toContainText("浦东");
  const calls = await page.evaluate(() => (window as any).__TEST_CALLS__);
  const submit = calls.find((c: any) => c.command === "submit_task");
  expect(submit.args.rules.fixedCells[0].address).toBe("B3");
  expect(submit.args.rules.sources[0].headerRow).toBe(2);
  expect(
    submit.args.rules.columns.find((c: any) => c.key === "金额").dataType,
  ).toBe("number");
  expect(
    calls.find((c: any) => c.command === "export_dataset").args.query.filters[0]
      .values,
  ).toEqual(["浦东"]);
  expect(errors).toEqual([]);
  await page.screenshot({ path: ".cache/workbench-result.png" });
});
test("keyboard focus returns after dialog and layout fits 1366x768", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "设置", exact: true }),
  ).toBeFocused();
  const size = await page.evaluate(() => ({
    width: document.documentElement.scrollWidth,
    height: document.documentElement.scrollHeight,
  }));
  expect(size.width).toBeLessThanOrEqual(1366);
  expect(size.height).toBeLessThanOrEqual(768);
  await page.screenshot({ path: ".cache/workbench-empty.png" });
});


test("normal files merge directly without confirming unchanged suggestions", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "添加文件", exact: true }).click();
  await expect(page.getByText("已选 2 个文件 · 约 6 条")).toBeVisible();
  await expect(page.locator(".rules-panel")).toBeVisible();
  await page.getByRole("button", { name: "格式建议" }).click();
  await expect(page.getByText("样本中没有需要调整的格式。")).toBeVisible();
  await expect(page.locator(".format-review")).toHaveCount(0);
  await expect(page.getByText(/第 1 行.*第 1 行/)).toHaveCount(0);
  await page.getByRole("button", { name: "按当前设置合并" }).click();
  await expect(page.locator(".pane-tabs button.active")).toContainText("汇总表", { timeout: 12000 });
  await page.getByRole("button", { name: "保存到电脑", exact: true }).click();
  const calls = await page.evaluate(() => (window as any).__TEST_CALLS__);
  expect(calls.find((c: any) => c.command === "submit_task").args.rules.columns).toEqual([]);
  expect(calls.some((c: any) => c.command === "export_dataset")).toBe(true);
  await page.getByRole("button", { name: "分类统计", exact: true }).click();
  await expect(page.locator('.stage-nav [aria-current="step"]')).toContainText("预览与保存");
  await expect(page.getByRole("button", { name: "返回修改配置", exact: true })).toBeVisible();
  await page.getByLabel("1. 按什么分类？").selectOption("门店");
  await page.getByLabel("2. 要统计什么？").selectOption("金额");
  await page.getByRole("button", { name: "计算统计", exact: true }).click();
  await expect(page.getByRole("button", { name: "保存统计表", exact: true })).toBeEnabled();
  const analysisCalls = await page.evaluate(() => (window as any).__TEST_CALLS__);
  expect(analysisCalls.find((c: any) => c.command === "aggregate").args.spec).toMatchObject({ rowFields: ["门店"], valueFields: ["金额"], columnFields: [], aggregation: "sum" });
});

test("selected format adjustments are included in the immediately submitted task", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "添加文件", exact: true }).click();
  await page.evaluate(() => { (window as any).__TEST_CURRENCY_HINT__ = true; });
  await page.getByRole("button", { name: "格式建议" }).click();
  await expect(page.getByText("改成数字，可以求和和计算平均值")).toBeVisible();
  await expect(page.getByLabel("调整 金额")).not.toBeChecked();
  await page.getByLabel("调整 金额").check();
  await page.getByRole("button", { name: "调整后合并表格" }).click();
  await expect(page.locator(".pane-tabs button.active")).toContainText("汇总表", { timeout: 12000 });
  const calls = await page.evaluate(() => (window as any).__TEST_CALLS__);
  const task = calls.find((c: any) => c.command === "submit_task");
  expect(task.args.rules.columns.find((c: any) => c.key === "金额").dataType).toBe("number");
  expect(task.args.rules.columns.find((c: any) => c.key === "门店").dataType).toBe("auto");
});

test("saved settings are discoverable and can be reused", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "添加文件", exact: true }).click();
  await page.getByRole("button", { name: "配置文件", exact: true }).click();
  await expect(page.getByText("还没有保存过设置，可以先保存这次的。")).toBeVisible();
  await page.getByRole("button", { name: "保存这次的设置", exact: true }).click();
  await page.getByRole("button", { name: "保存设置", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("button", { name: "配置文件", exact: true }).click();
  await page.getByRole("button", { name: "使用所选设置", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const calls = await page.evaluate(() => (window as any).__TEST_CALLS__);
  expect(calls.find((c: any) => c.command === "apply_scheme").args.schemeId).toBe("saved1");
});


test("rectangle selection supports multirow headers and returning to configuration preserves them", async ({ page }) => {
  await page.goto("/");await page.getByRole("button",{name:"添加文件",exact:true}).click();
  const a=await page.locator('.excel-cell[data-cell="A1"]').boundingBox();
  const b=await page.locator('.excel-cell[data-cell="B2"]').boundingBox();
  await page.mouse.move(a!.x+12,a!.y+12);await page.mouse.down();await page.mouse.move(b!.x+12,b!.y+12,{steps:8});await page.mouse.up();
  await expect(page.locator(".excel-address")).toHaveText("A1:B2");
  await page.getByRole("button",{name:"设为表头",exact:true}).click();
  await page.getByRole("button",{name:"使用这片表头",exact:true}).click();
  await page.getByRole("button",{name:"预览结果",exact:true}).click();
  await expect(page.locator(".pane-tabs button.active")).toContainText("汇总表",{timeout:12000});
  await page.getByRole("button",{name:"返回修改配置",exact:true}).click();
  await expect(page.locator(".excel-address")).toHaveText("A1:B2");
  await page.getByRole("button",{name:"表头",exact:true}).click();
  await expect(page.getByRole("button",{name:"A1:B2 · 修改",exact:true})).toBeVisible();
  await page.getByLabel("附加来源文件名").uncheck();
  await page.getByRole("button",{name:"预览与保存",exact:true}).click();
  await expect(page.getByText("配置已修改，当前结果仍是修改前的版本。")).toBeVisible();
  await page.setViewportSize({ width: 1024,height: 640 });
  expect(await page.locator(".app-header").evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
  expect(await page.locator(".result-toolbar").evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
  const calls=await page.evaluate(() => (window as any).__TEST_CALLS__);
  expect(calls.find((c:any) => c.command==="submit_task").args.rules.sources.every((s:any) => s.headerRange.startRow===1&&s.headerRange.endRow===2&&s.headerRange.endColumn===2)).toBe(true);
  await page.screenshot({path:".cache/staged-results.png"});
});

test("import disclosure is reachable by keyboard and restores focus", async ({ page }) => {
  await page.goto("/");await page.getByRole("button",{ name: "添加文件",exact: true }).click();
  const disclosure = page.getByLabel("更多导入方式");
  await disclosure.focus();await page.keyboard.press("Enter");
  await expect(page.getByRole("button",{ name: "添加文件夹",exact: true })).toBeVisible();
  await page.keyboard.press("Tab");await expect(page.getByRole("button",{ name: "添加文件夹",exact: true })).toBeFocused();
  await page.keyboard.press("Escape");await expect(disclosure).toBeFocused();
  await expect(page.locator(".import-menu")).not.toHaveAttribute("open");
});

test("compact files show shared details once and preserve selection and deletion", async ({ page }) => {
  await page.goto("/");await page.evaluate(() => { (window as any).__TEST_COMMON_PREFIX__ = true; });
  await page.getByRole("button",{ name: "添加文件",exact: true }).click();
  await expect(page.locator(".source-file-caption")).toContainText("销售");
  await expect(page.locator(".source-file-caption")).toContainText("每表 3 条 · 2 列");
  await expect(page.locator(".source-file-preview").first()).toHaveText("BJ01_北京");
  await expect(page.locator(".file-meta, .sidebar-bottom")).toHaveCount(0);
  const second = page.getByRole("button",{ name: "预览 销售_BJ02_北京.xlsx / 销售",exact: true });
  await expect(second).toHaveAttribute("title",/销售_BJ02_北京.xlsx.*销售 · 3 条 · 2 列.*2.0 KB/s);
  await second.click();await expect(second).toHaveAttribute("aria-current","true");
  await page.getByRole("checkbox",{ name: "选择 销售_BJ02_北京.xlsx / 销售",exact: true }).uncheck();
  expect(await page.getByRole("checkbox",{ name: "全选工作表",exact: true }).evaluate((el) => (el as HTMLInputElement).indeterminate)).toBe(true);
  await page.getByRole("checkbox",{ name: "全选工作表",exact: true }).check();
  await expect(page.getByRole("checkbox",{ name: "选择 销售_BJ02_北京.xlsx / 销售",exact: true })).toBeChecked();
  const remove = page.getByRole("button",{ name: "删除 销售_BJ02_北京.xlsx",exact: true });
  await remove.focus();await expect(remove).toHaveCSS("opacity","1");await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog")).toContainText("导入文件副本");
  await page.keyboard.press("Escape");await expect(remove).toBeFocused();
});

test("multi-sheet files expand on demand and support partial and whole-file selection", async ({ page }) => {
  await page.goto("/");await page.evaluate(() => { (window as any).__TEST_MULTI_SHEET__ = true; });
  await page.getByRole("button",{ name: "添加文件",exact: true }).click();
  await expect(page.getByRole("checkbox",{ name: "选择 2月.xlsx / 部门明细",exact: true })).toHaveCount(0);
  await page.getByRole("button",{ name: "展开 2月.xlsx",exact: true }).click();
  const child = page.getByRole("checkbox",{ name: "选择 2月.xlsx / 部门明细",exact: true });
  await child.uncheck();
  const parent = page.getByRole("checkbox",{ name: "选择 2月.xlsx 的全部工作表",exact: true });
  expect(await parent.evaluate((el) => (el as HTMLInputElement).indeterminate)).toBe(true);
  await parent.check();await expect(child).toBeChecked();
  await page.getByRole("button",{ name: "预览 2月.xlsx / 部门明细",exact: true }).click();
  await expect(page.locator(".source-context strong")).toHaveText("部门明细");
  await page.getByRole("button",{ name: "收起 2月.xlsx",exact: true }).click();
  await page.getByRole("button",{ name: "预览 2月.xlsx",exact: true }).click();
  await expect(child).toBeVisible();await expect(page.locator(".source-context strong")).toHaveText("销售");
});

test("long file lists scroll inside the sidebar and do not shrink the workspace", async ({ page }) => {
  await page.goto("/");await page.evaluate(() => { (window as any).__TEST_MANY_FILES__ = true; });
  await page.getByRole("button",{ name: "添加文件",exact: true }).click();
  await expect(page.locator(".source-file-row")).toHaveCount(32);
  const layout = await page.evaluate(() => ({ width: document.documentElement.clientWidth, height: document.documentElement.scrollHeight, listHeight: document.querySelector(".file-scroll")!.clientHeight, listContent: document.querySelector(".file-scroll")!.scrollHeight }));
  expect(layout.width).toBe(1366);expect(layout.height).toBe(768);
  expect(layout.listContent).toBeGreaterThan(layout.listHeight);
});

test("keyboard selection moves past merged cells and selects a header row", async ({ page }) => {
  await page.goto("/");await page.evaluate(() => { (window as any).__TEST_MERGED__=true; });
  await page.getByRole("button",{name:"添加文件",exact:true}).click();
  await expect(page.locator('.excel-cell[data-cell="A1"]')).toHaveAttribute("aria-rowspan","2");
  await page.locator(".excel-scroll").focus();await page.keyboard.press("Control+Home");await page.keyboard.press("ArrowDown");
  await expect(page.locator(".excel-address")).toHaveText("A3");
  await page.keyboard.press("Shift+ArrowRight");await expect(page.locator(".excel-address")).toHaveText("A3:B3");
  await page.keyboard.press("Shift+Space");
  await page.getByRole("button",{name:"设为表头",exact:true}).click();
  await expect(page.getByRole("dialog")).toContainText("已框选 A3:B3");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button",{name:"设为表头",exact:true})).toBeFocused();
});

test("file sidebar width can be resized by dragging resizer and reset on double click", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "添加文件", exact: true }).click();
  const resizer = page.locator(".sidebar-resizer");
  await expect(resizer).toBeVisible();

  const boxBefore = await page.locator(".file-sidebar").boundingBox();
  expect(boxBefore).not.toBeNull();

  // Drag resizer to expand sidebar
  const resizerBox = await resizer.boundingBox();
  expect(resizerBox).not.toBeNull();
  await page.mouse.move(resizerBox!.x + 3, resizerBox!.y + 100);
  await page.mouse.down();
  await page.mouse.move(resizerBox!.x + 100, resizerBox!.y + 100, { steps: 5 });
  await page.mouse.up();

  const boxAfter = await page.locator(".file-sidebar").boundingBox();
  expect(boxAfter!.width).toBeGreaterThan(boxBefore!.width + 50);

  // Double click resets to default (216px)
  await resizer.dblclick();
  const boxReset = await page.locator(".file-sidebar").boundingBox();
  expect(Math.round(boxReset!.width)).toBe(216);
});

test("settings modal provides tab switching to about tab with sponsor QR code", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByText("设置与数据")).toBeVisible();
  await expect(page.getByText("关于")).toBeVisible();

  // Switch to about tab
  await page.getByRole("button", { name: "关于", exact: true }).click();
  await expect(page.getByRole("heading", { name: "汇格 · GridFlow" })).toBeVisible();
  await expect(page.getByText("v2.0.0")).toBeVisible();
  await expect(page.getByText("赞赏项目")).toBeVisible();
  await expect(page.getByText("微信赞赏")).toBeVisible();
  await expect(page.locator(".about-qr-image")).toBeVisible();

  // Close about tab
  await page.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("welcome screen provides sample data and onboarding tour", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    localStorage.removeItem("gridflow_tour_completed");
  });
  await expect(page.getByText("体验示例数据（2份连锁门店月报）")).toBeVisible();
  await expect(page.getByRole("button", { name: "快速上手教程" })).toBeVisible();

  // Open Help modal
  await page.getByRole("button", { name: "快速上手教程" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByText("帮助与新手指引")).toBeVisible();
  await expect(page.getByText("4 步极速上手")).toBeVisible();
  await expect(page.getByRole("button", { name: "进阶与实用技巧" })).toBeVisible();
  await expect(page.getByRole("button", { name: "快捷键速查" })).toBeVisible();

  // Close Help modal
  await page.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);

  // Click sample data to import 2 stores
  await page.getByText("体验示例数据（2份连锁门店月报）").click();

  // Tour popover should be visible and start at step 1
  await expect(page.locator(".tour-popover")).toBeVisible();
  await expect(page.locator(".tour-step-badge")).toContainText("核心指引 1 / 4");
  await expect(page.getByText("多表集中管理")).toBeVisible();

  // Next step (2/4: Set header)
  await page.getByRole("button", { name: "下一步" }).click();
  await expect(page.locator(".tour-step-badge")).toContainText("核心指引 2 / 4");
  await expect(page.getByText("框选并设定表头")).toBeVisible();

  // Next step (3/4: Add column)
  await page.getByRole("button", { name: "下一步" }).click();
  await expect(page.locator(".tour-step-badge")).toContainText("核心指引 3 / 4");
  await expect(page.getByText("孤立信息一键平铺")).toBeVisible();

  // Next step (4/4: Preview and merge)
  await page.getByRole("button", { name: "下一步" }).click();
  await expect(page.locator(".tour-step-badge")).toContainText("核心指引 4 / 4");
  await expect(page.getByText("毫秒级合并与导出")).toBeVisible();

  // Complete tour
  await page.getByRole("button", { name: "完成指引" }).click();
  await expect(page.locator(".tour-popover")).toHaveCount(0);
});

test("supports batch deletion when all or multiple files are selected", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    (window as any).__TEST_MANY_FILES__ = true;
  });
  await page.getByText("体验示例数据（2份连锁门店月报）").click();

  // All 32 files imported, file-batch-delete button should be visible in sidebar section-title
  const batchDeleteBtn = page.locator(".file-batch-delete");
  await expect(batchDeleteBtn).toBeVisible();
  await expect(batchDeleteBtn).toContainText("全部删除 (32)");

  // Click batch delete button to trigger confirmation modal
  await batchDeleteBtn.click();
  await expect(page.locator("dialog.modal")).toBeVisible();
  await expect(page.getByText("确认删除 32 个导入文件副本？")).toBeVisible();

  // Confirm delete
  await page.getByRole("button", { name: "确认删除" }).click();
  await expect(page.locator("dialog.modal")).toHaveCount(0);

  // After deleting all files, it should automatically return to initial welcome state
  await expect(page.locator(".welcome")).toBeVisible();
  await expect(page.getByText("把多份表格，合成一份 Excel。")).toBeVisible();
});

test("home icon in top right allows returning to initial page state and resetting", async ({ page }) => {
  await page.goto("/");
  await page.getByText("体验示例数据（2份连锁门店月报）").click();

  // Now in configure mode with 2 files loaded
  await expect(page.locator(".file-sidebar")).toBeVisible();

  // Top right should have the Home button
  const homeBtn = page.getByRole("button", { name: "回到主页" });
  await expect(homeBtn).toBeVisible();

  // Click home button -> switches to initial page state (welcome)
  await homeBtn.click();
  await expect(page.locator(".welcome")).toBeVisible();
  await expect(page.getByText("把多份表格，合成一份 Excel。")).toBeVisible();
  await expect(page.locator(".welcome-active-session")).toBeVisible();
  await expect(page.getByText("当前工作区已导入 2 个文件")).toBeVisible();

  // Can click continue to go back to configuration
  await page.getByRole("button", { name: "继续配置汇总" }).click();
  await expect(page.locator(".file-sidebar")).toBeVisible();

  // Return to home again, and click reset
  await homeBtn.click();
  await expect(page.locator(".welcome-active-session")).toBeVisible();
  await page.getByRole("button", { name: "清空并重置" }).click();

  // Confirm reset modal appears
  await expect(page.locator("dialog.modal")).toBeVisible();
  await expect(page.getByText("确认清空当前导入的 2 个文件并恢复初始页面状态？")).toBeVisible();
  await page.getByRole("button", { name: "确认删除" }).click();
  await expect(page.locator("dialog.modal")).toHaveCount(0);

  // Workspace cleared, pure initial page state
  await expect(page.locator(".welcome")).toBeVisible();
  await expect(page.locator(".welcome-active-session")).toHaveCount(0);
});

test("preview stays visible and does not revert to empty result state during polling", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "添加文件", exact: true }).click();
  await page
    .getByRole("button", { name: "预览结果", exact: true })
    .first()
    .click();

  await expect(page.locator(".pane-tabs button.active")).toContainText(
    "汇总表",
    { timeout: 12000 },
  );

  // Click "合并预览" tab
  await page.getByRole("button", { name: "合并预览", exact: true }).click();
  await expect(page.getByRole("button", { name: "合并预览", exact: true })).toHaveClass(/active/);

  // Wait 1.5s to let polling ticks fire
  await page.waitForTimeout(1500);

  // "合并预览" tab must still be active and NOT hijacked back to empty "汇总表"
  await expect(page.getByRole("button", { name: "合并预览", exact: true })).toHaveClass(/active/);
  await expect(page.locator(".empty-state h2")).not.toHaveText("配置完成后，在这里查看结果");
});
