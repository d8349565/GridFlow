import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ArrowDown, ArrowUp, Check, ChevronLeft, ChevronRight, Download, FileSpreadsheet, FolderOpen, Layers, Pencil, Plus, RotateCcw, Save, Scissors, Square, Trash2 } from "lucide-react";
import { call, chooseFiles, isTauri, queryFor } from "./api";
import { Button, Grid } from "./components";
import ExcelPreview, { rangeLabel } from "./ExcelPreview";
import WorkflowBar from "./WorkflowBar";
import type { DataPage, Dataset, DatasetQuery, Filter, SheetRange, SourceFile, SplitEdit, SplitGroup, SplitNamePart, SplitOutput, SplitPlan, SplitProgress, SplitRequest, SplitSourcePage } from "./types";
import "./split.css";

export type SplitSession = {
  step: number; mode: "summary" | "import"; imported: SourceFile | null; sheetId: string; header: SheetRange | null;
  dataset: Dataset | null; query: DatasetQuery | null; field: string; columns: string[];
  naming: SplitNamePart[]; separator: string; format: string; date: string;
  edits: Record<string, SplitEdit>; output: SplitOutput;
};
const recipeKey = "gridflow_split_recipe_v1";
const dateText = () => new Date().toLocaleDateString("sv-SE").replaceAll("-", "");
const basename = (name: string) => name.replace(/\.(xlsx|xls|csv)$/i, "");
const textError = (e: unknown) => e && typeof e === "object" && "message" in e ? String(e.message) : String(e);
const displayPath = (path: string) => path.startsWith("\\\\?\\UNC\\") ? "\\\\" + path.slice(8) : path.replace(/^\\\\\?\\/, "");
const visibleColumns = (dataset: Dataset) => dataset.columns.filter(c => !dataset.rules.columns.some(rule => rule.key === c.key && rule.hidden));
const blankSession = (): SplitSession => ({ step: 0, mode: "import", imported: null, sheetId: "", header: null, dataset: null, query: null,
  field: "", columns: [], naming: [{ kind: "value" }], separator: "_", format: "xlsx", date: dateText(), edits: {},
  output: { directory: "", newFolder: true, folderName: "表格拆分" } });
export function splitSessionForDataset(dataset: Dataset, query: DatasetQuery): SplitSession {
  return { ...blankSession(), mode: "summary", step: 1, dataset, query: { ...query, offset: 0 }, columns: visibleColumns(dataset).map(c => c.key),
    output: { directory: "", newFolder: true, folderName: batchName(dataset.name) } };
}
function batchName(name: string) { return `${basename(name)}_拆分_${dateText()}_${new Date().toTimeString().slice(0, 8).replaceAll(":", "")}`; }
const groupLabel = (g: SplitGroup) => g.kind === 0 ? "（空值）" : g.value;
const partLabel = (part: SplitNamePart) => ({ value: "分类值", text: "固定文字", source: "原表名", sheet: "工作表名", date: "命名日期", sequence: "序号", column: "其他字段" })[part.kind];
const operatorLabels: Record<string, string> = { contains: "包含", eq: "等于", ne: "不等于", gt: "大于", gte: "大于等于", lt: "小于", lte: "小于等于", empty: "为空", notEmpty: "不为空", in: "属于以下值" };
function isNamingPart(part: unknown): part is SplitNamePart {
  if (!part || typeof part !== "object" || !("kind" in part)) return false;
  if (["value", "source", "sheet", "date", "sequence"].includes(String(part.kind))) return true;
  return part.kind === "text" ? "text" in part && typeof part.text === "string" : part.kind === "column" && "key" in part && typeof part.key === "string";
}

export default function SplitWorkbench({ datasets, session, onSessionChange, onNavigate, onBusy }: {
  datasets: Dataset[]; session: SplitSession | null; onSessionChange: (session: SplitSession) => void;
  onNavigate: (tool: "summary" | "rename") => void; onBusy: (busy: boolean) => void;
}) {
  const [draft, setDraft] = useState<SplitSession>(() => session ?? { ...blankSession(), mode: datasets.some(d => !d.sample) ? "summary" : "import" });
  const [plan, setPlan] = useState<SplitPlan | null>(null);
  const [preparedKey, setPreparedKey] = useState("");
  const [preparing, setPreparing] = useState(false);
  const [busy, setBusy] = useState("");
  const busyRef = useRef(false);
  const [error, setError] = useState("");
  const [info, setInfo] = useState("");
  const [progress, setProgress] = useState<SplitProgress | null>(null);
  const [reload, setReload] = useState(0);
  const [selection, setSelection] = useState<SheetRange | null>(null);
  const [page, setPage] = useState(0);
  const [groupSearch, setGroupSearch] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [detailGroup, setDetailGroup] = useState<string | null>(null);
  const [view, setView] = useState<"files" | "data">("files");
  const queue = useRef<Promise<void>>(Promise.resolve());
  const prepareEpoch = useRef(0);
  const candidates = datasets.filter(d => !d.sample);
  const sheet = draft.imported?.sheets.find(s => s.id === draft.sheetId);
  const columns = draft.dataset ? visibleColumns(draft.dataset) : [];
  const request = useMemo<SplitRequest | null>(() => draft.query && draft.dataset && draft.field ? {
    query: draft.query, field: draft.field, columns: draft.columns, naming: draft.naming, separator: draft.separator, format: draft.format, date: draft.date,
    sourceName: draft.mode === "import" ? basename(draft.imported?.name ?? draft.dataset.name) : draft.dataset.name,
    sheetName: draft.mode === "import" ? sheet?.name ?? draft.dataset.name : draft.dataset.name,
  } : null, [draft.query, draft.dataset, draft.field, draft.columns, draft.naming, draft.separator, draft.format, draft.date, draft.mode, draft.imported, sheet]);
  const planKey = JSON.stringify([request, draft.edits, draft.output]);
  const planReady = !!plan && preparedKey === planKey && !preparing;
  const selected = plan?.groups.filter(g => g.selected) ?? [];
  const problems = selected.filter(g => g.error);
  const saved = selected.filter(g => g.state === "saved");
  const unfinished = selected.filter(g => g.state !== "saved");
  const filteredGroups = plan?.groups.filter(g => !groupSearch || groupLabel(g).toLocaleLowerCase().includes(groupSearch.toLocaleLowerCase())) ?? [];
  const currentGroups = filteredGroups.slice(page * 100, (page + 1) * 100);
  const currentDetail = plan?.groups.find(g => g.id === detailGroup);
  const locked = !!busy;
  const canExport = planReady && !locked && selected.length > 0 && problems.length === 0 && !!plan?.directory && unfinished.length > 0;
  const completed = !!plan?.groups.some(g => g.state !== "ready");

  useEffect(() => { onSessionChange(draft); }, [draft]);
  useEffect(() => { onBusy(!!busy || preparing); return () => onBusy(false); }, [busy, preparing]);
  useEffect(() => {
    const blocked = () => setError("表格仍在处理中。可先点击“停止”，处理结束后再关闭窗口。");
    window.addEventListener("gridflow-split-close-blocked", blocked);
    return () => window.removeEventListener("gridflow-split-close-blocked", blocked);
  }, []);
  useEffect(() => {
    const epoch = ++prepareEpoch.current;
    if (!request) { setPlan(null); setPreparing(false); return; }
    let disposed = false;
    setPreparing(true); setError("");
    const timer = setTimeout(() => {
      // Queue only the most recent edit after the previous request completes.
      queue.current = queue.current.catch(() => {}).then(async () => {
        if (disposed || epoch !== prepareEpoch.current) return;
        try {
          const next = await call<SplitPlan>("split_prepare", { request, edits: Object.values(draft.edits), output: draft.output });
          if (!disposed && epoch === prepareEpoch.current) { setPlan(next); setPreparedKey(planKey); setPage(old => Math.min(old, Math.max(0, Math.ceil(next.groups.length / 100) - 1))); setDetailGroup(old => next.groups.some(g => g.id === old) ? old : null); }
        } catch (e) { if (!disposed && epoch === prepareEpoch.current) { setError(textError(e)); setPlan(null); } }
        finally { if (!disposed && epoch === prepareEpoch.current) setPreparing(false); }
      });
    }, 350);
    return () => { disposed = true; clearTimeout(timer); };
  }, [planKey, reload]);
  useEffect(() => {
    if (!busy && !preparing) return;
    let alive = true, pending = false;
    const read = async () => {
      if (pending) return; pending = true;
      try { const value = await call<SplitProgress>("split_progress"); if (alive) setProgress(value); }
      catch {} finally { pending = false; }
    };
    void read(); const timer = setInterval(() => void read(), 500);
    return () => { alive = false; clearInterval(timer); };
  }, [busy, preparing]);
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false, unlisten: (() => void) | undefined;
    void getCurrentWebviewWindow().onDragDropEvent(e => {
      if (e.payload.type === "drop" && !busyRef.current) {
        if (e.payload.paths.length !== 1) setError("每次拆分一张表，请拖入一个 Excel / CSV 文件。");
        else void importPath(e.payload.paths[0]);
      }
    }).then(stop => { if (disposed) stop(); else unlisten = stop; }).catch(e => setError(textError(e)));
    return () => { disposed = true; unlisten?.(); };
  }, []);

  function patch(value: Partial<SplitSession>) { setDraft(old => ({ ...old, ...value })); setInfo(""); }
  async function run(label: string, action: () => Promise<void>) {
    if (busyRef.current) return;
    busyRef.current = true; setBusy(label); setError(""); setInfo("");
    try { await queue.current; await action(); } catch (e) { setError(textError(e)); }
    finally { busyRef.current = false; setBusy(""); }
  }
  async function importPath(path: string) {
    await run("导入表格", async () => {
      const file = await call<SourceFile>("split_import", { path });
      const first = file.sheets[0];
      patch({ mode: "import", imported: file, sheetId: first.id, header: { startRow: first.suggestedHeaderRow, endRow: first.suggestedHeaderRow, startColumn: 1, endColumn: first.width },
        dataset: null, query: null, field: "", columns: [], edits: {}, step: 0,
        output: { ...draft.output, folderName: batchName(file.name) } });
      setSelection(null); setPlan(null); setDetailGroup(null);
    });
  }
  async function chooseInput() { try { const paths = await chooseFiles(false, false, false); if (paths[0]) await importPath(paths[0]); } catch (e) { setError(textError(e)); } }
  function useDataset(id: string) {
    const dataset = candidates.find(d => d.id === id);
    if (!dataset) return;
    patch({ dataset, query: queryFor(id), field: "", columns: visibleColumns(dataset).map(c => c.key), edits: {},
      output: { ...draft.output, folderName: batchName(dataset.name) } });
    setDetailGroup(null);
  }
  async function confirmSource() {
    if (draft.mode === "summary") { patch({ step: 1 }); return; }
    if (!sheet || !draft.header) return;
    await run("确认数据范围", async () => {
      const dataset = await call<Dataset>("split_use_sheet", { sheetId: sheet.id, range: draft.header });
      patch({ dataset, query: queryFor(dataset.id), field: "", columns: dataset.columns.map(c => c.key), edits: {}, step: 1 });
      setDetailGroup(null);
    });
  }
  function changeMode(mode: SplitSession["mode"]) {
    if (mode !== draft.mode) { patch({ mode, dataset: null, query: null, field: "", columns: [], edits: {}, step: 0 }); setDetailGroup(null); }
  }
  function changeSheet(id: string) {
    const next = draft.imported?.sheets.find(s => s.id === id);
    if (!next) return;
    patch({ sheetId: id, header: { startRow: next.suggestedHeaderRow, endRow: next.suggestedHeaderRow, startColumn: 1, endColumn: next.width }, dataset: null, query: null, field: "", edits: {} });
    setSelection(null);
  }
  function changeHeader(header: SheetRange) {
    patch({ header, dataset: null, query: null, field: "", columns: [], edits: {} });
    setDetailGroup(null);
  }
  function changeSorts(sorts: DatasetQuery["sorts"]) {
    if (draft.query && JSON.stringify(sorts) !== JSON.stringify(draft.query.sorts)) patch({ query: { ...draft.query, sorts, offset: 0 } });
  }
  function editGroup(group: SplitGroup, value: Partial<SplitEdit>) {
    patch({ edits: { ...draft.edits, [group.id]: { id: group.id, selected: group.selected, name: draft.edits[group.id]?.name ?? null, ...value } } });
  }
  function selectAll(selected: boolean) {
    patch({ edits: { ...draft.edits, ...Object.fromEntries((plan?.groups ?? []).map(g => [g.id, { id: g.id, selected, name: draft.edits[g.id]?.name ?? null }])) } });
  }
  function changePart(i: number, part: SplitNamePart) { patch({ naming: draft.naming.map((v, n) => n === i ? part : v) }); }
  function movePart(i: number, offset: number) {
    const naming = [...draft.naming]; [naming[i], naming[i + offset]] = [naming[i + offset], naming[i]]; patch({ naming });
  }
  function changeFilter(i: number, patchFilter: Partial<Filter>) {
    if (draft.query) patch({ query: { ...draft.query, offset: 0, filters: draft.query.filters.map((f, n) => n === i ? { ...f, ...patchFilter } : f) }, edits: {} });
  }
  function saveRecipe() {
    try { localStorage.setItem(recipeKey, JSON.stringify({ field: draft.field, columns: draft.columns, naming: draft.naming, separator: draft.separator, format: draft.format })); setInfo("拆分字段、导出列和命名规则已保存，可在其他表格中套用。"); }
    catch (e) { setError(`规则未能保存：${textError(e)}`); }
  }
  function applyRecipe() {
    try {
      const recipe = JSON.parse(localStorage.getItem(recipeKey) ?? "null");
      if (!recipe) { setInfo("还没有保存的拆分规则。设置好后点击“保存规则”。"); return; }
      if (!Array.isArray(recipe.naming) || !recipe.naming.length || recipe.naming.length > 64 || !recipe.naming.every(isNamingPart)
        || typeof recipe.separator !== "string" || !["xlsx", "csv"].includes(recipe.format) || !Array.isArray(recipe.columns)) throw new Error("保存的规则无法识别，请重新保存");
      const known = new Set(columns.map(c => c.key));
      const field = known.has(recipe.field) ? recipe.field as string : "";
      const keep = recipe.columns.filter((key: unknown): key is string => typeof key === "string" && known.has(key));
      patch({ field, columns: keep.length ? keep : columns.map(c => c.key), naming: recipe.naming, separator: recipe.separator, format: recipe.format, edits: {} });
      setInfo(field ? "已套用规则，正在重新核对分类和文件名。" : "已套用命名规则。当前表中没有原拆分字段，请重新选择。");
    } catch (e) { setError(textError(e)); }
  }
  async function exportFiles() {
    if (!canExport || !plan) return;
    await run("导出拆分文件", async () => {
      const next = await call<SplitPlan>("split_execute", { planId: plan.id }); setPlan(next);
      const count = next.groups.filter(g => g.state === "saved").length;
      const failed = next.groups.filter(g => g.state === "failed").length;
      const stopped = next.groups.filter(g => g.selected && g.state === "cancelled").length;
      setInfo(`已保存 ${count} 个文件${failed ? `，${failed} 个失败` : ""}${stopped ? `，${stopped} 个未完成` : ""}。${failed || stopped ? "可查看清单并重试未完成项。" : "可打开文件夹查看结果。"}`);
    });
  }
  async function stop() {
    if (preparing && !busy) { ++prepareEpoch.current; setPreparing(false); setPreparedKey(""); setInfo("已停止核对，可点击“重新核对”继续。"); }
    else setInfo("正在停止，已保存的文件会保留。");
    try { await call("split_cancel"); } catch (e) { setError(textError(e)); }
  }
  const hint = busy ? `正在${busy}，请稍候。` : preparing ? "正在核对完整数据的分组和文件名…" : !draft.dataset && draft.step > 0 ? "请返回选择数据。"
    : draft.step === 0 ? draft.mode === "import" ? "选择工作表并确认表头；可以在原表拖选表头区域。" : "选择一张完整汇总表，或导入已有 Excel。"
    : !draft.field ? "选择用于分类的字段，就能看到每份文件的数据行数和名称。"
    : problems.length ? `有 ${problems.length} 个所选文件需要调整命名，请查看清单中的提示。`
    : draft.step === 2 && !plan?.directory ? "选择保存文件夹，然后批量导出所选文件。"
    : planReady ? `筛选后 ${plan!.totalRows.toLocaleString()} 行 · ${plan!.groups.length.toLocaleString()} 个分类 · 已选 ${selected.length.toLocaleString()} 个文件、${plan!.selectedRows.toLocaleString()} 行`
    : "设置完成后，可核对拆分清单并导出。";

  return <div className="app-shell split-shell">
    <header className="app-header">
      <button type="button" className="brand" disabled={locked || preparing} title="回到汇总，保留拆分设置" onClick={() => onNavigate("summary")}><div className="app-mark"><FileSpreadsheet size={18} /></div><strong>汇格 · GridFlow</strong></button>
      <nav className="module-nav" aria-label="工具入口"><button disabled={locked || preparing} onClick={() => onNavigate("summary")}>Excel 汇总</button><button disabled={locked || preparing} onClick={() => onNavigate("rename")}>批量重命名</button><button className="active" aria-current="page">表格拆分</button></nav>
      <div className="header-actions"><Button icon={Save} className="quiet" disabled={!draft.dataset || locked} onClick={saveRecipe}>保存规则</Button><Button icon={RotateCcw} className="quiet" disabled={!draft.dataset || locked} onClick={applyRecipe}>套用规则</Button></div>
    </header>
    <WorkflowBar label="拆分步骤" current={draft.step} steps={[
      { label: "选择数据", disabled: locked, onClick: () => patch({ step: 0 }) },
      { label: "设置拆分与命名", disabled: !draft.dataset || locked, onClick: () => patch({ step: 1 }) },
      { label: "核对并导出", disabled: !planReady || locked, onClick: () => patch({ step: 2 }) },
    ]} hint={hint} actions={locked || preparing ? <Button icon={Square} onClick={() => void stop()}>停止</Button> : draft.step === 0 ?
      <Button primary icon={ChevronRight} disabled={draft.mode === "summary" ? !draft.dataset : !sheet || !draft.header} onClick={() => void confirmSource()}>使用此表，设置拆分</Button> : draft.step === 1 ?
      <Button primary icon={Check} disabled={!planReady || !selected.length || !!problems.length} onClick={() => patch({ step: 2 })}>下一步：核对并导出{selected.length ? `（${selected.length}）` : ""}</Button> :
      <><Button className="quiet" onClick={() => patch({ step: 1 })}>返回修改</Button><Button primary icon={Download} disabled={!canExport} onClick={() => void exportFiles()}>{completed ? `重试未完成项（${unfinished.length}）` : `导出所选文件（${selected.length}）`}</Button></>} />
    {(error || info || plan?.note) && <div className={`split-notice ${error ? "error" : ""}`} role={error ? "alert" : "status"}><span>{error || info || plan?.note}</span>{!locked && !preparing && draft.field && (error || !planReady) && <Button className="quiet" onClick={() => { setInfo(""); setReload(v => v + 1); }}>重新核对</Button>}</div>}
    {(locked || preparing) && <div className="split-progress" role="status"><span>{progress?.busy ? progress.phase : "正在核对拆分结果"}{progress?.current ? ` · ${progress.current}` : ""}</span>{!!progress?.total && <><progress value={progress.done} max={progress.total} aria-label="拆分处理进度" /><span>{progress.done} / {progress.total}</span></>}</div>}
    <div className={`split-workspace ${draft.step === 0 ? "split-choosing" : ""}`}>
      <aside className="split-settings" aria-label={draft.step === 0 ? "选择数据来源" : draft.step === 1 ? "拆分和命名设置" : "导出设置"}>
        {draft.step === 0 ? <>
          <div className="split-section"><h2>选择要拆分的表格</h2><div className="split-source-tabs"><Button primary={draft.mode === "summary"} disabled={locked} onClick={() => changeMode("summary")}>使用汇总结果</Button><Button primary={draft.mode === "import"} disabled={locked} onClick={() => changeMode("import")}>导入 Excel</Button></div>
            {draft.mode === "summary" ? <>{candidates.length ? <label>选择汇总表<select aria-label="选择汇总表" value={draft.dataset?.id ?? ""} disabled={locked} onChange={e => useDataset(e.target.value)}><option value="">请选择汇总表</option>{candidates.map(d => <option key={d.id} value={d.id}>{d.name} · {d.summary.outputRows.toLocaleString()} 行</option>)}</select></label> : <p className="muted">还没有完整汇总结果。可以先导入已有 Excel，或前往“Excel 汇总”生成结果。</p>}</> : <><Button icon={FolderOpen} disabled={locked} onClick={() => void chooseInput()}>{draft.imported ? "更换文件" : "选择 Excel / CSV 文件"}</Button><small>也可拖入一个文件。读取副本用于拆分。</small>{draft.imported && <><p className="split-source-name" title={draft.imported.name}>{draft.imported.name}</p><label>工作表<select aria-label="选择工作表" value={draft.sheetId} disabled={locked} onChange={e => changeSheet(e.target.value)}>{draft.imported.sheets.map(s => <option value={s.id} key={s.id}>{s.name} · {s.rows.toLocaleString()} 行</option>)}</select></label></>}</>}
          </div>
          {draft.mode === "import" && sheet && <div className="split-section"><h3>确认表头</h3><p>在右侧拖选表头，支持一行或多行。</p><Button disabled={!selection || locked} icon={Check} onClick={() => selection && changeHeader(selection)}>将选区设为表头</Button><label>表头起始行<input type="number" min={1} max={sheet.rows} value={draft.header?.startRow ?? 1} disabled={locked} onChange={e => { const row = Number(e.target.value); if (row >= 1 && row <= sheet.rows) changeHeader({ startRow: row, endRow: row, startColumn: 1, endColumn: sheet.width }); }} /></label><p className="split-header-label">当前表头：{draft.header ? rangeLabel(draft.header) : "未确认"}</p><small>数据从表头下一行开始，按所选表头列范围读取。</small></div>}
          {draft.dataset && draft.mode === "summary" && <div className="split-section"><h3>当前数据范围</h3><p>{draft.dataset.summary.outputRows.toLocaleString()} 行 · {columns.length} 列</p><small>{draft.query?.filters.length || draft.query?.search ? "已带入汇总页的筛选条件，下一步可查看和调整。" : "使用这张表的完整数据。"}</small></div>}
        </> : draft.step === 1 ? <>
          <div className="split-section"><h2>按字段拆分</h2><p className="split-source-name" title={draft.dataset?.name}>{draft.dataset?.name}</p><label>拆分字段<select aria-label="拆分字段" value={draft.field} disabled={locked} onChange={e => { patch({ field: e.target.value, edits: {} }); setDetailGroup(null); }}><option value="">选择用于分类的一列</option>{columns.map(c => <option key={c.key} value={c.key}>{c.name}</option>)}</select></label><small>每个不同值输出一个文件，空值单独成组。</small></div>
          <details className="split-section" open={draft.query?.filters.length ? true : undefined}><summary>筛选数据{draft.query?.filters.length ? `（${draft.query.filters.length} 项）` : "（可选）"}</summary>
            <label>查找匹配行<input aria-label="查找匹配行" value={draft.query?.search ?? ""} disabled={locked} placeholder="在整张表中查找" onChange={e => draft.query && patch({ query: { ...draft.query, search: e.target.value, offset: 0 }, edits: {} })} /></label>
            {draft.query?.filters.map((filter, i) => <div className="split-filter" key={i}><div><select aria-label={`筛选字段 ${i + 1}`} value={filter.key} disabled={locked} onChange={e => changeFilter(i, { key: e.target.value })}>{columns.map(c => <option key={c.key} value={c.key}>{c.name}</option>)}</select><Button icon={Trash2} aria-label={`移除筛选 ${i + 1}`} disabled={locked} onClick={() => draft.query && patch({ query: { ...draft.query, filters: draft.query.filters.filter((_, n) => n !== i) }, edits: {} })} /></div><select aria-label={`筛选方式 ${i + 1}`} value={filter.operator} disabled={locked} onChange={e => changeFilter(i, { operator: e.target.value })}>{Object.entries(operatorLabels).map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select>{!["empty", "notEmpty"].includes(filter.operator) && (filter.operator === "in" ? <label>每行一个值<textarea aria-label={`筛选值 ${i + 1}`} disabled={locked} value={filter.values.join("\n")} onChange={e => changeFilter(i, { values: e.target.value.split("\n") })} /></label> : <input aria-label={`筛选值 ${i + 1}`} disabled={locked} value={filter.values[0] ?? ""} placeholder="输入比较值" onChange={e => changeFilter(i, { values: [e.target.value] })} />)}</div>)}
            <Button icon={Plus} className="quiet" disabled={locked || !columns.length} onClick={() => draft.query && patch({ query: { ...draft.query, filters: [...draft.query.filters, { key: columns[0].key, operator: "contains", values: [""] }] }, edits: {} })}>添加筛选条件</Button>
          </details>
          <details className="split-section"><summary>选择导出列（{draft.columns.length} / {columns.length}）</summary><div className="split-column-list">{columns.map(c => <label className="check-label" key={c.key}><input type="checkbox" checked={draft.columns.includes(c.key)} disabled={locked} onChange={e => patch({ columns: e.target.checked ? columns.filter(v => v.key === c.key || draft.columns.includes(v.key)).map(v => v.key) : draft.columns.filter(k => k !== c.key) })} />{c.name}</label>)}</div></details>
          <div className="split-section"><h2>组成文件名</h2><small>点击添加内容，再调整顺序。完整名称显示在右侧。</small><div className="split-add-parts">{(["value", "text", "source", "sheet", "date", "sequence", "column"] as const).map(kind => <Button className="quiet" key={kind} disabled={locked || draft.naming.length >= 64} onClick={() => patch({ naming: [...draft.naming, kind === "text" ? { kind, text: "" } : kind === "column" ? { kind, key: columns[0]?.key ?? "" } : { kind }] })}>{partLabel(kind === "text" ? { kind, text: "" } : kind === "column" ? { kind, key: "" } : { kind })}</Button>)}</div>
            <ol className="split-name-parts">{draft.naming.map((part, i) => <li key={i}><div className="split-part-heading"><span>{i + 1}. {partLabel(part)}</span><div><Button icon={ArrowUp} aria-label={`上移命名项 ${i + 1}`} disabled={locked || i === 0} onClick={() => movePart(i, -1)} /><Button icon={ArrowDown} aria-label={`下移命名项 ${i + 1}`} disabled={locked || i === draft.naming.length - 1} onClick={() => movePart(i, 1)} /><Button icon={Trash2} aria-label={`移除命名项 ${i + 1}`} disabled={locked} onClick={() => patch({ naming: draft.naming.filter((_, n) => n !== i) })} /></div></div>{part.kind === "text" && <input aria-label={`固定文字 ${i + 1}`} value={part.text} placeholder="例如：费用明细" disabled={locked} onChange={e => changePart(i, { ...part, text: e.target.value })} />}{part.kind === "column" && <><select aria-label={`命名字段 ${i + 1}`} value={part.key} disabled={locked} onChange={e => changePart(i, { ...part, key: e.target.value })}>{columns.map(c => <option key={c.key} value={c.key}>{c.name}</option>)}</select><small>仅在该分类中只有一个值时使用。</small></>}</li>)}</ol>
            <div className="split-inline-fields"><label>内容之间的分隔符<input aria-label="文件名分隔符" value={draft.separator} maxLength={16} disabled={locked} placeholder="不添加" onChange={e => patch({ separator: e.target.value })} /></label><label>导出格式<select aria-label="导出格式" value={draft.format} disabled={locked} onChange={e => patch({ format: e.target.value })}><option value="xlsx">Excel（.xlsx）</option><option value="csv">CSV（.csv）</option></select></label></div>
            {draft.naming.some(p => p.kind === "date") && <label>命名日期<input aria-label="命名日期" type="date" disabled={locked} value={`${draft.date.slice(0, 4)}-${draft.date.slice(4, 6)}-${draft.date.slice(6, 8)}`} onChange={e => e.target.value && patch({ date: e.target.value.replaceAll("-", "") })} /></label>}
          </div>
        </> : <>
          <div className="split-section"><h2>核对并导出</h2><p>{draft.dataset?.name}</p><p>按“{columns.find(c => c.key === draft.field)?.name}”拆分</p><small>已选 {selected.length} 个文件 · {plan?.selectedRows.toLocaleString() ?? 0} 行 · {draft.columns.length} 列</small><Button className="quiet" icon={Pencil} disabled={locked} onClick={() => patch({ step: 1 })}>修改筛选或命名规则</Button></div>
          <div className="split-section"><h3>保存位置</h3><Button icon={FolderOpen} disabled={locked} onClick={() => void (async () => { try { const dirs = await chooseFiles(true, false, false); if (dirs[0]) patch({ output: { ...draft.output, directory: dirs[0] } }); } catch (e) { setError(textError(e)); } })()}>{draft.output.directory ? "更换保存文件夹" : "选择保存文件夹"}</Button>{draft.output.directory && <p className="split-path">{displayPath(draft.output.directory)}</p>}<label className="check-label"><input type="checkbox" checked={draft.output.newFolder} disabled={locked} onChange={e => patch({ output: { ...draft.output, newFolder: e.target.checked } })} />为本次拆分新建子文件夹</label>{draft.output.newFolder && <label>子文件夹名称<input aria-label="子文件夹名称" value={draft.output.folderName} disabled={locked} onChange={e => patch({ output: { ...draft.output, folderName: e.target.value } })} /></label>}{plan?.directory && <><small>最终保存位置</small><p className="split-path">{displayPath(plan.directory)}</p></>}<small>同名文件自动编号，最终名称以右侧清单为准。</small></div>
          {completed && <div className="split-section"><h3>处理结果</h3><p>已保存 {saved.length} 个 · 未完成 {unfinished.length} 个</p>{!!saved.length && <Button icon={FolderOpen} disabled={locked} onClick={() => void call("open_path", { path: plan?.directory, select: false }).catch(e => setError(textError(e)))}>打开文件夹</Button>}</div>}
        </>}
      </aside>
      <main className="split-main">
        {draft.step === 0 ? <>{draft.mode === "import" && sheet ? <ExcelPreview key={sheet.id} sheet={sheet} selection={selection} header={draft.header} onSelect={range => setSelection(range)} caption="拖选表头区域，再点击“将选区设为表头”" loadPage={async (offset, limit) => { const data = await call<SplitSourcePage>("split_source_page", { sheetId: sheet.id, offset, limit }); return [data.page, data.appearance]; }} /> : draft.dataset && draft.query ? <><div className="pane-toolbar"><strong>{draft.dataset.name}</strong><span>完整汇总数据</span></div><Grid columns={columns} version={draft.dataset.id} load={(offset, limit, sorts) => call<DataPage>("query_dataset", { query: { ...draft.query, offset, limit, sorts: sorts.length ? sorts : draft.query!.sorts } })} /></> : <div className="empty-state split-empty"><Scissors size={34} aria-hidden="true" /><h2>把一张总表，按分类拆成多个文件</h2><p>选择已有汇总结果，或拖入一个 Excel / CSV 文件。</p>{draft.mode === "import" && <Button primary icon={FolderOpen} disabled={locked} onClick={() => void chooseInput()}>选择文件</Button>}</div>}</> : <>
          <div className="pane-toolbar split-preview-toolbar"><div className="pane-tabs"><button type="button" className={view === "files" ? "active" : ""} onClick={() => setView("files")}>文件清单{plan ? `（${plan.groups.length}）` : ""}</button><button type="button" className={view === "data" ? "active" : ""} disabled={!planReady} onClick={() => { setDetailGroup(null); setView("data"); }}>数据预览</button></div><span>{preparing ? "正在更新清单…" : planReady ? `覆盖筛选后的全部 ${plan!.totalRows.toLocaleString()} 行` : "选择字段后自动生成预览"}</span></div>
          {view === "data" && planReady && plan ? <><div className="split-data-heading"><strong>{currentDetail ? `${groupLabel(currentDetail)} · ${currentDetail.rows.toLocaleString()} 行` : `筛选后的全部数据 · ${plan.totalRows.toLocaleString()} 行`}</strong><Button className="quiet" onClick={() => { setDetailGroup(null); setView("files"); }}>返回文件清单</Button></div><Grid columns={draft.dataset!.columns.filter(c => draft.columns.includes(c.key))} version={`${plan.id}:${detailGroup}`} initialSorts={draft.query?.sorts} onSort={changeSorts} load={(offset, limit, sorts) => call<DataPage>("split_page", { planId: plan.id, groupId: detailGroup, offset, limit, sorts })} /></> : plan ? <>
            <div className="split-list-actions"><label className="sr-only" htmlFor="split-find-group">查找分类</label><input id="split-find-group" placeholder="查找分类值" value={groupSearch} onChange={e => { setGroupSearch(e.target.value); setPage(0); }} /><Button className="quiet" disabled={locked || preparing} onClick={() => selectAll(true)}>全选全部</Button><Button className="quiet" disabled={locked || preparing} onClick={() => selectAll(false)}>全不选</Button><span>已选 {selected.length} / {plan.groups.length} 个文件</span></div>
            <div className="split-table-scroll" aria-busy={preparing}><table className="split-table"><thead><tr><th>导出</th><th>分类值</th><th className="numeric">数据行数</th><th>最终文件名</th><th>状态 / 说明</th></tr></thead><tbody>{currentGroups.map(group => <tr key={group.id} className={group.selected ? "" : "split-unselected"}><td><input type="checkbox" aria-label={`导出分类 ${groupLabel(group)}`} checked={draft.edits[group.id]?.selected ?? group.selected} disabled={locked} onChange={e => editGroup(group, { selected: e.target.checked })} /></td><td><button type="button" className="split-value" disabled={!planReady || locked} title="查看该分类的明细" onClick={() => { setDetailGroup(group.id); setView("data"); }}>{groupLabel(group)}</button>{group.kind !== 0 && <small>{group.kind === 2 ? "数值" : group.kind === 4 ? "日期" : group.kind === 3 ? "布尔值" : "文本"}</small>}</td><td className="numeric">{group.rows.toLocaleString()}</td><td><div className="split-filename"><span>{group.filename}</span><Button className="quiet" icon={Pencil} aria-label={`修改 ${groupLabel(group)} 的文件名`} disabled={locked} onClick={() => setEditingId(editingId === group.id ? null : group.id)} /></div>{editingId === group.id && <div className="split-custom-name"><label>单独命名<input aria-label={`${groupLabel(group)} 的自定义文件名`} value={draft.edits[group.id]?.name ?? basename(group.filename)} disabled={locked} onChange={e => editGroup(group, { name: e.target.value })} /></label><small>扩展名 .{draft.format} 自动添加</small><Button className="quiet" disabled={locked} onClick={() => { editGroup(group, { name: null }); setEditingId(null); }}>恢复规则命名</Button></div>}</td><td><span className={group.error || group.state === "failed" ? "split-error-text" : group.state === "saved" ? "split-saved-text" : ""}>{group.error ?? (group.state === "saved" ? "已保存" : group.state === "failed" ? "导出失败" : group.state === "cancelled" ? "未完成" : group.selected ? "待导出" : "不导出")}</span>{group.note && <small>{group.note}</small>}</td></tr>)}</tbody></table>{!currentGroups.length && <div className="empty-state">{groupSearch ? "没有匹配的分类。" : "筛选后没有数据，请调整筛选条件。"}</div>}</div>
            <div className="split-pagination"><span>{filteredGroups.length.toLocaleString()} 个分类 · 每页 100 项</span><Button icon={ChevronLeft} disabled={page === 0} onClick={() => setPage(v => v - 1)}>上一页</Button><span>{filteredGroups.length ? page + 1 : 0} / {Math.ceil(filteredGroups.length / 100)}</span><Button icon={ChevronRight} disabled={(page + 1) * 100 >= filteredGroups.length} onClick={() => setPage(v => v + 1)}>下一页</Button></div>
          </> : <div className="empty-state split-empty"><Layers size={30} aria-hidden="true" /><h2>{preparing ? "正在核对完整数据…" : "选择拆分字段，查看文件清单"}</h2><p>每个分类的行数和最终文件名都会在这里显示。</p></div>}
        </>}
      </main>
    </div>
  </div>;
}
