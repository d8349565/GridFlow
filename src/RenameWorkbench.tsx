import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, Check, Download, FilePenLine, FileSpreadsheet, FolderOpen, History, Plus, RotateCcw, Save, Square, Trash2, X } from "lucide-react";
import { Button, Modal } from "./components";
import ExcelPreview, { rangeLabel } from "./ExcelPreview";
import { call, cellText, coordinate, isTauri, savePath } from "./api";
import type { Cell, DataPage, RenameCell, RenameCheck, RenameFile, RenameOutput, RenamePage, RenamePart, RenamePlan, RenamePreset, RenamePresetSummary, RenameProgress, RenameRow, RenameRules, RenameScan, SheetAppearance, SheetRange } from "./types";
import "./rename.css";
import { parseLookupTable } from "./renameTable";
import RenameConfigDialog from "./RenameConfigDialog";
import RenameRulesPanel, { type PickTarget } from "./RenameRulesPanel";
import RenamePanelLayout from "./RenamePanelLayout";
import RenameNotification, { type RenameNoticeData } from "./RenameNotification";
import RenamePresetDialog from "./RenamePresetDialog";

const emptyRules = (): RenameRules => ({ parts: [], separator: "_", prefix: "", suffix: "", checks: [] });
const label = (source: RenameCell) => `${source.sheet}!${coordinate(source.row, source.column)}`;
const stateNames: Record<string, string> = { ready: "待执行", skipped: "已跳过", unchanged: "无需修改", renamed: "已重命名", copied: "已复制", moved: "已移动", copyOnly: "原文件保留", failed: "失败", cancelled: "未执行", restored: "已撤销", undoFailed: "撤销失败", uncertain: "需人工核对" };
const problemStates = new Set(["skipped", "failed", "undoFailed", "uncertain", "copyOnly"]);
const hasProblem = (row: RenameRow) => problemStates.has(row.state) || row.hasMismatch;
const displayPath = (path: string) => path.startsWith("\\\\?\\UNC\\") ? "\\\\" + path.slice(8) : path.startsWith("\\\\?\\") ? path.slice(4) : path;
const errorText = (e: unknown) => e && typeof e === "object" && "message" in e ? String(e.message) : String(e);
const ruleKey = "gridflow_rename_rules_v1";
const fileName = (path:string) => path.split(/[\\/]/).at(-1) || path;
export type RenameSession = { selected: string[]; activeId: string; output?: RenameOutput };

function LookupDialog({ source, onAdd, onClose }: { source: RenameCell; onAdd: (parts: RenamePart[]) => void; onClose: () => void }) {
  const [text, setText] = useState("");
  const [header, setHeader] = useState(true);
  const [keyColumn, setKeyColumn] = useState(0);
  const [chosen, setChosen] = useState<number[]>([1]);
  const parsedTable = useMemo(() => parseLookupTable(text), [text]);
  const parsed = parsedTable.rows;
  const width = Math.max(0, ...parsed.map(row => row.length));
  const names = Array.from({ length: width }, (_, i) => header ? parsed[0]?.[i] || `第 ${i + 1} 列` : `第 ${i + 1} 列`);
  const data = header ? parsed.slice(1) : parsed;
  const selected = chosen.filter(c => c !== keyColumn && c < width);
  const keys = data.map(row => row[keyColumn]?.trim());
  const invalid = parsedTable.error || (!text.trim() ? "" : width < 2 ? "请从 Excel 复制至少两列，列之间保留制表符。" : !data.length ? "标题下面还需要至少一行对照内容。" : keys.some(k => !k) || new Set(keys).size !== keys.length ? "编码列有空值或重复项，请修正后添加。" : data.some(row => selected.some(c => !row[c]?.trim())) ? "选中的对照列有空值，请补齐后添加。" : "");
  return <Modal title="粘贴对照信息" onClose={onClose} wide><div className="modal-body rename-dialog-body">
    <p>用 <strong>{label(source)}</strong> 的内容查找对照表。直接从 Excel 复制编码和需要的几列信息，一次添加到文件名。</p>
    <label className="rename-field">对照表内容<textarea autoFocus rows={6} placeholder={"编码\t客户简称\t生产线名称\n001\t客户甲\t一号生产线"} value={text} onChange={e => setText(e.target.value)} /></label>
    <label className="check-label"><input type="checkbox" checked={header} onChange={e => setHeader(e.target.checked)} />首行是标题</label>
    {width >= 2 && <><label className="rename-field">用哪一列查找<select value={keyColumn} onChange={e => setKeyColumn(Number(e.target.value))}>{names.map((name, i) => <option key={i} value={i}>{name}</option>)}</select></label><div className="rename-lookup-options"><strong>把哪些信息加入文件名</strong>{names.map((name, i) => i !== keyColumn && <label className="check-label" key={i}><input type="checkbox" checked={chosen.includes(i)} onChange={e => setChosen(old => e.target.checked ? [...old, i] : old.filter(v => v !== i))} />{name}</label>)}</div><div className="rename-table-scroll"><table><thead><tr>{names.map((n, i) => <th key={i}>{n}</th>)}</tr></thead><tbody>{data.slice(0, 5).map((row, i) => <tr key={i}>{names.map((_, c) => <td key={c}>{row[c]}</td>)}</tr>)}</tbody></table></div><small>共 {data.length} 行，预览前 5 行。找不到对照时默认跳过该文件，可在添加后设置备用文字。</small></>}
    {invalid && <p className="notice error" role="alert">{invalid}</p>}
  </div><div className="modal-footer"><Button onClick={onClose}>取消</Button><Button primary disabled={!text.trim() || !!invalid || !selected.length} onClick={() => { onAdd(selected.map(c => ({ kind: "lookup", source, name: names[c], entries: data.map(row => ({ key: row[keyColumn].trim(), value: row[c].trim() })), fallback: null }))); onClose(); }}>添加 {selected.length || ""} 项对照信息</Button></div></Modal>;
}

export default function RenameWorkbench({ onBack, onBusy, session, onSessionChange, onFlush }: { onBack: () => void; onBusy: (busy: boolean) => void; session: RenameSession | null; onSessionChange: (session: RenameSession) => void; onFlush: (flush: (() => Promise<void>) | null) => void }) {
  const [files, setFiles] = useState<RenameFile[]>([]);
  const [selected, setSelected] = useState<string[]>(session?.selected ?? []);
  const [activeId, setActiveId] = useState(session?.activeId ?? "");
  const initialized = useRef(session !== null);
  const [sheetNames, setSheetNames] = useState<string[]>([]);
  const [sheetName, setSheetName] = useState("");
  const [firstPage, setFirstPage] = useState<RenamePage | null>(null);
  const [sheetError, setSheetError] = useState("");
  const [selection, setSelection] = useState<SheetRange | null>(null);
  const [value, setValue] = useState<Cell | null>(null);
  const [recursive, setRecursive] = useState(true);
  const [output, setOutput] = useState<RenameOutput>(session?.output ?? { mode: "original", directory: null, numberConflicts: true });
  const [presetDialog, setPresetDialog] = useState<"save" | "list" | "new" | null>(null);
  const [currentPreset, setCurrentPreset] = useState<RenamePresetSummary | null>(null);
  const [rules, setRules] = useState<RenameRules>(() => { try { const v = JSON.parse(localStorage.getItem(ruleKey) || "null"); return v && Array.isArray(v.parts) && Array.isArray(v.checks) ? v : emptyRules(); } catch { return emptyRules(); } });
  const [draftReady, setDraftReady] = useState(false);
  const [draftError, setDraftError] = useState("");
  const [draftAttempt, setDraftAttempt] = useState(0);
  const draftQueue = useRef<Promise<unknown>>(Promise.resolve());
  const draftTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latestDraft = useRef({rules,ready:draftReady}); latestDraft.current={rules,ready:draftReady};
  const [configuration, setConfiguration] = useState<{ mode:"save" | "load"; path:string } | null>(null);
  const [closeError, setCloseError] = useState("");
  const [exitAfterSave, setExitAfterSave] = useState(false);
  const [live, setLive] = useState<RenameRow | null>(null);
  const [livePending, setLivePending] = useState(false);
  const [liveError, setLiveError] = useState("");
  const [busy, setBusy] = useState("");
  const cancellable = ["扫描文件", "核对全部文件名", "执行重命名", "撤销上次处理"].includes(busy);
  const task = useRef(false);
  const [progress, setProgress] = useState<RenameProgress | null>(null);
  const [notice, setNotice] = useState<RenameNoticeData | null>(null);
  const clearNotice=useCallback(() => setNotice(null),[]);
  const notify=useCallback((kind:RenameNoticeData["kind"],title:string,detail?:string,path?:string) => setNotice({kind,title,detail,path}),[]);
  const [plan, setPlan] = useState<RenamePlan | null>(null);
  const [view, setView] = useState<"configure" | "review">("configure");
  const [confirm, setConfirm] = useState<"execute" | "undo" | "clear" | null>(null);
  const [lookupSource, setLookupSource] = useState<RenameCell | null>(null);
  const [pick, setPick] = useState<PickTarget | null>(null);
  const editor = useRef<HTMLElement>(null);
  const sheetPreview = useRef<HTMLDivElement>(null);
  const [pageIndex, setPageIndex] = useState(0);
  const [filePage, setFilePage] = useState(0);
  const [onlyProblems, setOnlyProblems] = useState(false);
  const [epoch, setEpoch] = useState(0);
  const activeFile = files.find(f => f.id === activeId);
  const source: RenameCell | null = selection && sheetName && selection.startRow === selection.endRow && selection.startColumn === selection.endColumn ? { sheet: sheetName, row: selection.startRow, column: selection.startColumn } : null;
  const readyCount = plan?.rows.filter(r => r.state === "ready").length ?? 0;
  const undoCount = plan?.rows.filter(r => ["renamed", "copied", "moved", "copyOnly", "undoFailed"].includes(r.state)).length ?? 0;
  const processedCount = plan?.rows.filter(r => ["renamed", "copied", "moved", "copyOnly"].includes(r.state)).length ?? 0;
  const concentrated = plan?.output.mode !== undefined && plan.output.mode !== "original";
  const rows = plan?.rows.filter(r => !onlyProblems || hasProblem(r)) ?? [];
  const incompleteCheck = rules.checks.some(check => !check.sums.length);
  const validSelection = !!selection && firstPage?.sheet.fileId === activeId && firstPage.sheet.name === sheetName;
  const canConfirmPick = validSelection && (pick?.kind === "sum" || (!!source && value !== null));
  const pickInstruction = pick?.kind === "tag" ? "请点选决定是否显示标签的单元格。" : pick?.kind === "expected" ? "请点选填写总金额或总数量的单元格。" : pick?.kind === "sumCondition" ? "请点选决定是否计入此区域的单元格。" : "请框选合计区域；选择单个单元格时，将从该行合计到表尾。";
  const pickConfirmText = pick?.kind === "tag" ? "确认条件单元格" : pick?.kind === "expected" ? "确认填写值" : pick?.kind === "sumCondition" ? "使用此条件" : "使用此合计区域";
  useEffect(() => {
    if (!pick) return;
    sheetPreview.current?.querySelector<HTMLElement>(".excel-scroll")?.focus();
    if (pick.kind !== "tag" && pick.check !== undefined) editor.current?.querySelectorAll(".rename-check")[pick.check]?.scrollIntoView({ block:"nearest" });
    const cancel=(e: KeyboardEvent) => { if(e.key === "Escape") { e.preventDefault(); setPick(null); } };
    document.addEventListener("keydown",cancel); return () => document.removeEventListener("keydown",cancel);
  }, [pick]);
  function startPick(target: PickTarget) { setSelection(null); setValue(null); setPick(target); }
  function change(next: RenameRules) { setRules(next); setPlan(null); setLive(null); setLiveError(""); }
  function partAt(index: number, part: RenamePart) { change({ ...rules, parts: rules.parts.map((p, i) => i === index ? part : p) }); }
  function checkAt(index: number, check: RenameCheck) { change({ ...rules, checks: rules.checks.map((p, i) => i === index ? check : p) }); }
  function addPart(part: RenamePart) { change({ ...rules, parts: [...rules.parts, part] }); }
  const persistDraft = useCallback((next: RenameRules) => {
    const operation=draftQueue.current.catch(() => {}).then(() => call("rename_save_draft",{rules:next})).then(() => { localStorage.removeItem(ruleKey); });
    draftQueue.current=operation; return operation;
  }, []);
  const flushDraft = useCallback(async () => {
    if (!latestDraft.current.ready) return;
    if (draftTimer.current !== null) clearTimeout(draftTimer.current);
    await persistDraft(latestDraft.current.rules);
  }, [persistDraft]);
  useEffect(() => { onFlush(flushDraft); return () => onFlush(null); }, [onFlush,flushDraft]);
  async function leave() { await run("加密保存规则",async () => { await flushDraft(); onBack(); }); }
  async function run(name: string, work: () => Promise<void>) {
    if (task.current) return;
    task.current = true; onBusy(true); setBusy(name); setNotice(null); setProgress(null);
    try { await work(); } catch (e) { notify(e && typeof e === "object" && "code" in e && e.code === "CANCELLED" ? "info" : "error",`${name}未完成`,errorText(e)); }
    finally { task.current = false; onBusy(false); setBusy(""); }
  }
  async function refreshFiles() {
    const next = await call<RenameFile[]>("rename_files"); setFiles(next);
    const first = !initialized.current; initialized.current = true;
    setSelected(old => first ? next.map(f => f.id) : old.filter(id => next.some(f => f.id === id)));
    setActiveId(old => next.some(f => f.id === old) ? old : next[0]?.id ?? ""); setEpoch(v => v + 1);
  }
  useEffect(() => { void refreshFiles().catch(e => notify("error","读取文件列表失败",errorText(e))); }, []);
  useEffect(() => { if (initialized.current) onSessionChange({ selected, activeId, output }); }, [selected, activeId, output, onSessionChange]);
  useEffect(() => {
    const savedId=localStorage.getItem("gridflow_rename_preset_id");
    if (savedId) void call<RenamePresetSummary[]>("rename_presets").then(list => setCurrentPreset(list.find(p => p.id === savedId) ?? null)).catch(() => {});
  }, []);
  useEffect(() => {
    let disposed=false; setDraftReady(false); setDraftError("");
    void (async () => {
      const encrypted=await call<RenameRules | null>("rename_load_draft");
      const legacy=localStorage.getItem(ruleKey);
      const next=legacy ? JSON.parse(legacy) as RenameRules : encrypted ?? emptyRules();
      if (legacy) { await call("rename_save_draft",{rules:next}); localStorage.removeItem(ruleKey); }
      if (!disposed) { setRules(next); setDraftReady(true); }
    })().catch(e => { if (!disposed) setDraftError(errorText(e)); });
    return () => { disposed=true; };
  }, [draftAttempt]);
  useEffect(() => {
    if (!draftReady) return;
    draftTimer.current=setTimeout(() => {
      void persistDraft(rules).catch(e => notify("error","加密自动保存失败",`${errorText(e)}。请先保存加密配置。`));
    },450);
    return () => { if(draftTimer.current !== null) clearTimeout(draftTimer.current); };
  }, [rules,draftReady,persistDraft]);
  useEffect(() => {
    const blocked = () => notify("info","任务正在处理中","请先点击“停止”，等待当前文件处理结束后再关闭。");
    const failed = (event: Event) => setCloseError(errorText((event as CustomEvent<unknown>).detail));
    window.addEventListener("gridflow-rename-close-blocked", blocked);
    window.addEventListener("gridflow-rename-save-failed", failed);
    return () => { window.removeEventListener("gridflow-rename-close-blocked", blocked); window.removeEventListener("gridflow-rename-save-failed", failed); };
  }, []);
  async function retryClose() { await run("保存并退出",async () => { try { await flushDraft(); await getCurrentWindow().destroy(); } catch(e) { setCloseError(errorText(e)); } }); }
  useEffect(() => {
    if (!busy) return;
    let disposed = false;
    const timer = setInterval(() => void call<RenameProgress>("rename_progress").then(p => { if (!disposed) setProgress(p); }).catch(() => {}), 350);
    return () => { disposed = true; clearInterval(timer); };
  }, [busy]);
  useEffect(() => {
    if (!activeId) return;
    let disposed = false; setSheetNames([]); setSheetName(""); setFirstPage(null); setSheetError(""); setSelection(null); setValue(null);
    call<string[]>("rename_sheet_names", { fileId: activeId }).then(names => { if (!disposed) { setSheetNames(names); setSheetName(old => names.includes(old) ? old : names[0] ?? ""); } }).catch(e => { if (!disposed) setSheetError(errorText(e)); });
    return () => { disposed = true; };
  }, [activeId, epoch]);
  useEffect(() => {
    if (!activeId || !sheetName) return;
    let disposed = false; setFirstPage(null); setSelection(null); setValue(null); setSheetError("");
    call<RenamePage>("rename_page", { fileId: activeId, sheet: sheetName, offset: 0, limit: 100 }).then(p => { if (!disposed) setFirstPage(p); }).catch(e => { if (!disposed) setSheetError(errorText(e)); });
    return () => { disposed = true; };
  }, [activeId, sheetName, epoch]);
  const loadPage = useCallback(async (offset: number, limit: number): Promise<[DataPage, SheetAppearance]> => {
    const p = offset === 0 && firstPage ? firstPage : await call<RenamePage>("rename_page", { fileId: activeId, sheet: sheetName, offset, limit });
    return [p.page, p.appearance];
  }, [activeId, sheetName, firstPage]);
  useEffect(() => {
    setLive(null); setLiveError("");
    if (!activeId || !rules.parts.length || busy || !draftReady || incompleteCheck || view !== "configure") { setLivePending(false); return; }
    let disposed = false; setLivePending(true);
    const timer = setTimeout(() => { call<RenameRow>("rename_live", { fileId: activeId, rules }).then(v => { if (!disposed) setLive(v); }).catch(e => { if (!disposed) setLiveError(errorText(e)); }).finally(() => { if (!disposed) setLivePending(false); }); }, 300);
    return () => { disposed = true; clearTimeout(timer); };
  }, [activeId, rules, busy, view, epoch, draftReady, incompleteCheck]);
  const addFiles = useRef<(paths: string[]) => Promise<void>>(async () => {});
  addFiles.current = async paths => run("扫描文件", async () => {
    const result = await call<RenameScan>("rename_scan", { paths: [...files.map(f => f.path), ...paths], recursive });
    const existing = new Set(files.map(f => f.id));
    setFiles(result.files); setSelected(result.files.filter(f => !existing.has(f.id) || selected.includes(f.id)).map(f => f.id)); setActiveId(old => result.files.some(f => f.id === old) ? old : result.files[0]?.id ?? ""); setFilePage(0); setPlan(null); setView("configure");
    if(result.warnings.length) notify("warning",`${result.warnings.length} 个位置未能读取`,result.warnings.join("\n"));
    else if(!result.files.length) notify("warning","没有找到 Excel 文件","支持 .xlsx 和 .xls，临时文件会自动忽略。");
  });
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false; let stop: (() => void) | undefined;
    getCurrentWebviewWindow().onDragDropEvent(e => { if (e.payload.type === "drop" && !task.current) void addFiles.current(e.payload.paths); }).then(unlisten => { if (disposed) unlisten(); else stop = unlisten; });
    return () => { disposed = true; stop?.(); };
  }, []);
  async function choose(directory: boolean) {
    try {
      const paths = await open({ directory, multiple: true, filters: directory ? undefined : [{ name: "Excel 文件", extensions: ["xlsx", "xls"] }] });
      if (paths) await addFiles.current(Array.isArray(paths) ? paths : [paths]);
    } catch (e) { notify("error","添加文件未完成",errorText(e)); }
  }
  function confirmPick() {
    if (!pick || !selection || !canConfirmPick) return;
    if (pick.kind === "tag") {
      if (!source) return;
      const part=pick.part === undefined ? null : rules.parts[pick.part];
      if (pick.part === undefined) addPart({kind:"condition",condition:{source,operator:"positive",value:""},text:"【标签】"});
      else if (part?.kind === "condition") partAt(pick.part,{...part,condition:{...part.condition,source}});
    } else if (pick.kind === "expected") {
      if (!source) return;
      if (pick.check === undefined) {
        const index=rules.checks.length;
        change({...rules,checks:[...rules.checks,{expected:source,label:"金额异常",tolerancePercent:1,sums:[]}]});
        startPick({kind:"sum",check:index}); return;
      }
      const check=rules.checks[pick.check]; if (check) checkAt(pick.check,{...check,expected:source});
    } else {
      const check=rules.checks[pick.check]; if (!check) return;
      if (pick.kind === "sumCondition") {
        if (!source) return;
        checkAt(pick.check,{...check,sums:check.sums.map((s,i) => i === pick.sum ? {...s,condition:s.condition ? {...s.condition,source} : {source,operator:"positive",value:""}} : s)});
      } else {
        const area={sheet:sheetName,range:source ? {...selection,endRow:1048576} : selection};
        checkAt(pick.check,{...check,sums:pick.sum === undefined ? [...check.sums,{area,divisor:1,minFilled:0,condition:null}] : check.sums.map((s,i) => i === pick.sum ? {...s,area} : s)});
      }
    }
    setPick(null);
  }
  async function chooseConfiguration() { await run("选择配置",async () => {
    const path=await open({multiple:false,filters:[{name:"汇格加密命名配置",extensions:["gfrcfg"]}]});
    if (typeof path === "string") setConfiguration({mode:"load",path});
  }); }
  function rememberPreset(preset: RenamePresetSummary) { setCurrentPreset(preset); localStorage.setItem("gridflow_rename_preset_id",preset.id); }
  function usePreset(preset: RenamePreset) { change(preset.rules); rememberPreset(preset.summary); setPick(null); setDraftReady(true); setDraftError(""); setView("configure"); notify("success","已使用本机配置",preset.summary.name); }
  function configWorking(working: boolean) { task.current=working; onBusy(working); setBusy(working ? "保存或读取配置" : ""); }
  function changeOutput(next: RenameOutput) { setOutput(next); setPlan(null); }
  async function chooseOutput() { await run("选择输出文件夹",async () => {const path=await open({directory:true,multiple:false,title:"选择集中保存的文件夹"});if(typeof path === "string") changeOutput({...output,directory:path});}); }
  async function prepare() { await run("核对全部文件名", async () => { const p = await call<RenamePlan>("rename_prepare", { ids: selected, rules, output }); setPlan(p); setPageIndex(0); setOnlyProblems(false); setView("review"); }); }
  async function execute() { if (!plan) return; setConfirm(null); await run("执行重命名", async () => { const p = await call<RenamePlan>("rename_execute", { planId: plan.id }); setPlan(p); await refreshFiles(); }); }
  async function undo() { if (!plan) return; setConfirm(null); await run("撤销上次处理", async () => { const p = await call<RenamePlan>("rename_undo", { runId: plan.id }); setPlan(p); await refreshFiles(); }); }
  async function history() { await run("读取上次记录", async () => { const p = await call<RenamePlan | null>("rename_latest"); if (!p) { notify("info","暂无重命名记录","执行重命名后，可在这里查看和恢复。"); return; } setPlan(p); setView("review"); setPageIndex(0); setOnlyProblems(false); }); }
  async function exportLog() { if (!plan) return; await run("保存日志与汇总", async () => { const path = await savePath(`重命名日志与汇总_${new Date().toISOString().slice(0, 10)}.xlsx`, "xlsx"); if (path) { await call("rename_export", { plan, path }); notify("success","日志与汇总已保存",fileName(path),path); } }); }

  return <div className="app-shell rename-shell">
    <header className="app-header"><div className="brand"><div className="app-mark"><FileSpreadsheet size={18} /></div><strong>汇格 · GridFlow</strong></div><nav className="module-nav" aria-label="工具入口"><button disabled={!!busy} onClick={() => void leave()}>Excel 汇总</button><button className="active" aria-current="page">批量重命名</button></nav><div className="header-actions"><Button className="quiet" icon={Save} disabled={!!busy || !!pick || !draftReady} onClick={() => setPresetDialog("save")}>保存配置</Button><Button className="quiet" icon={FolderOpen} title={currentPreset ? `当前：${currentPreset.name}` : "选择本机已保存的配置，或导入、导出配置"} disabled={!!busy || !!pick || !draftReady} onClick={() => setPresetDialog("list")}>本机配置</Button><Button className="quiet" icon={History} disabled={!!busy} onClick={() => void history()}>上次记录与恢复</Button>{view === "configure" && <Button primary icon={Check} disabled={!!busy || !!pick || !draftReady || incompleteCheck || !selected.length || !rules.parts.length || (output.mode !== "original" && !output.directory)} onClick={() => void prepare()}>核对全部 {selected.length || ""} 个文件名</Button>}</div></header>
    {(notice || draftError) && <div className="rename-notifications">
      {draftError && <RenameNotification notice={{kind:"error",title:"加密草稿读取失败",detail:draftError}} onRetry={() => setDraftAttempt(v => v + 1)} />}
      {notice && <RenameNotification notice={notice} onClose={clearNotice} paused={!!configuration || !!presetDialog || !!confirm || !!closeError} />}
    </div>}
    {busy && <div className="rename-progress" role="status"><span>{progress?.busy ? progress.phase : busy} {progress?.total ? `${progress.done} / ${progress.total}` : ""}</span><span title={progress?.current}>{progress?.current}</span>{progress?.total ? <progress value={progress.done} max={progress.total} /> : <progress />}{cancellable && <Button icon={Square} disabled={!progress?.busy} onClick={() => void call("rename_cancel").then(() => notify("info","正在停止","已完成的结果和恢复记录会保留。")).catch(e => notify("error","停止任务未完成",errorText(e)))}>停止</Button>}</div>}
    {view === "configure" ? <RenamePanelLayout files={
      <aside id="rename-files-panel" className="rename-files"><h2>待重命名文件</h2><div className="rename-file-actions"><Button icon={Plus} disabled={!!busy} onClick={() => void choose(false)}>添加文件</Button><Button icon={FolderOpen} disabled={!!busy} onClick={() => void choose(true)}>文件夹</Button></div><label className="check-label"><input type="checkbox" disabled={!!busy} checked={recursive} onChange={e => setRecursive(e.target.checked)} />添加文件夹时包含子文件夹</label><div className="rename-file-count"><label className="check-label"><input type="checkbox" aria-label="全选待重命名文件" disabled={!!busy || !files.length} checked={!!files.length && selected.length === files.length} onChange={e => { setSelected(e.target.checked ? files.map(f => f.id) : []); setPlan(null); }} />已选 {selected.length} / {files.length}</label><Button className="quiet icon-button" icon={Trash2} aria-label="清空文件清单" title="仅移出清单，保留磁盘文件" disabled={!!busy || !files.length} onClick={() => void run("清空文件清单", async () => { await call("rename_scan", { paths: [], recursive: false }); setFiles([]); setSelected([]); setActiveId(""); setFirstPage(null); setPlan(null); })} /></div>
        <div className="rename-file-list">{files.slice(filePage * 100, (filePage + 1) * 100).map(f => <div key={f.id} className={f.id === activeId ? "active" : ""}><input type="checkbox" aria-label={`选择 ${f.name}`} checked={selected.includes(f.id)} disabled={!!busy} onChange={e => { setSelected(old => e.target.checked ? [...old, f.id] : old.filter(id => id !== f.id)); setPlan(null); }} /><button disabled={!!busy} title={f.path} onClick={() => setActiveId(f.id)}><FileSpreadsheet size={14} /><span>{f.name}</span></button></div>)}{!files.length && <p className="muted">添加或拖入 Excel 文件。<br />预览和核对不会修改文件。</p>}</div>
        {files.length > 100 && <div className="rename-pagination"><Button disabled={!filePage} onClick={() => setFilePage(v => v - 1)}>上一页</Button><span>{filePage + 1}/{Math.ceil(files.length / 100)}</span><Button disabled={(filePage + 1) * 100 >= files.length} onClick={() => setFilePage(v => v + 1)}>下一页</Button></div>}
        <section className="rename-output" aria-label="文件输出位置"><h3>文件存放位置</h3><label className="rename-field"><select aria-label="文件存放方式" disabled={!!busy} value={output.mode === "original" ? "original" : "collect"} onChange={e => changeOutput({...output,mode:e.target.value === "original" ? "original" : "copy"})}><option value="original">在原文件夹改名</option><option value="collect">集中到一个文件夹</option></select></label>
          {output.mode !== "original" ? <><Button icon={FolderOpen} className="rename-output-folder" disabled={!!busy} title={output.directory ? displayPath(output.directory) : "选择所有文件集中保存的位置"} onClick={() => void chooseOutput()}>{output.directory ? <span>{fileName(output.directory)}</span> : "选择目标文件夹"}</Button>{output.directory && <small className="rename-output-path" title={displayPath(output.directory)}>{displayPath(output.directory)}</small>}<label className="check-label"><input type="checkbox" disabled={!!busy} checked={output.mode === "copy"} onChange={e => changeOutput({...output,mode:e.target.checked ? "copy" : "move"})} />保留原文件（复制）</label><label className="check-label"><input type="checkbox" disabled={!!busy} checked={output.numberConflicts} onChange={e => changeOutput({...output,numberConflicts:e.target.checked})} />重名时自动编号</label><small>{output.mode === "copy" ? "重命名后复制到目标文件夹。" : "重命名后移到目标文件夹，原位置不再保留。"}执行前可核对完整路径。</small></> : <small>在各自所在的文件夹中重命名。</small>}
        </section>
      </aside>} source={<main className="rename-source"><div className="pane-toolbar"><strong title={activeFile?.path}>{activeFile?.name ?? "从原表中选择命名内容"}</strong>{sheetNames.length > 0 && <select aria-label="重命名取值工作表" disabled={!!busy} value={sheetName} onChange={e => setSheetName(e.target.value)}>{sheetNames.map(s => <option key={s}>{s}</option>)}</select>}</div>
        {pick ? <div className="rename-pick-banner" role="status"><div className="rename-pick-description"><strong>{pickInstruction}</strong><span>{validSelection && selection ? `${sheetName}!${rangeLabel(selection)}${source ? ` · ${cellText(value ?? undefined) || "空白"}` : ""}` : "尚未选择，支持切换工作表；按 Esc 可取消。"}</span></div><Button primary disabled={!!busy || !canConfirmPick} onClick={confirmPick}>{pickConfirmText}</Button><Button icon={X} aria-label="取消选取" onClick={() => setPick(null)} /></div> : <div className="rename-selection-actions"><span>{source ? `${label(source)} · ${cellText(value ?? undefined) || "空白"}` : "先选内容再加入文件名；条件和校验先点功能再选。"}</span><Button disabled={!!busy || !draftReady || !source} icon={Plus} primary onClick={() => source && addPart({ kind: "cell", source, decimals: null })}>加入文件名</Button><Button disabled={!!busy || !draftReady || !source} onClick={() => setLookupSource(source)}>添加对照</Button><Button disabled={!!busy || !draftReady || !firstPage} onClick={() => startPick({kind:"tag"})}>条件标签</Button><Button disabled={!!busy || !draftReady || !firstPage} onClick={() => startPick({kind:"expected"})}>添加校验</Button></div>}
        <div ref={sheetPreview} className={`rename-sheet ${pick ? "is-picking" : ""}`}>{sheetError ? <div className="empty-state"><p role="alert">{sheetError}</p><Button onClick={() => setEpoch(v => v + 1)}>重新读取</Button></div> : firstPage ? <ExcelPreview key={`${activeId}:${sheetName}:${epoch}`} sheet={firstPage.sheet} selection={selection} header={null} onSelect={(r, c) => { setSelection(r); setValue(c); }} loadPage={loadPage} caption="点选单元格命名 · 合并单元格取左上角 · 公式使用 Excel 已保存的结果" /> : <div className="empty-state">{activeId ? "正在读取工作表…" : <><FilePenLine size={36} /><h2>点选内容，批量生成文件名</h2><p>添加文件 → 点选单元格 → 核对并执行。<br />编码对照、金额/数量校验和条件标签按需添加。</p></>}</div>}</div>
        <section className="rename-live" aria-label="当前文件实时预览" aria-live="polite"><div><strong>当前文件名预览</strong><small>{livePending ? "正在更新…" : "随规则自动更新；执行前还会核对全部文件"}</small></div><p className={liveError || live?.state === "skipped" ? "rename-invalid" : ""}>{!draftReady ? "正在读取加密草稿…" : incompleteCheck ? "请选择校验的合计区域，完成后更新预览" : livePending ? "正在读取所选内容…" : liveError || live?.newName || (rules.parts.length ? live?.message || "选择文件查看预览" : "在上方点选单元格，加入文件名")}</p>{live && <><small>{stateNames[live.state]}{live.message ? ` · ${live.message}` : ""}</small>{!!live.checks.length && <details><summary>查看 {live.checks.length} 项校验结果</summary>{live.checks.map((c, i) => <p key={i}>{c}</p>)}</details>}</>}</section>
      </main>} rules={<RenameRulesPanel rules={rules} live={live} livePending={livePending} busy={!!busy} picking={!!pick} ready={draftReady} editorRef={editor} onChange={change} onPick={startPick} onReset={() => setConfirm("clear")} />
    } /> : <main className="rename-review"><div className="rename-review-heading"><div><Button className="quiet" icon={ArrowLeft} disabled={!!busy} onClick={() => setView("configure")}>返回命名规则</Button><h1>{plan?.journalPath ? "文件处理记录" : "核对文件名"}</h1><p>{plan?.journalPath ? `已完成 ${processedCount} 个 · 已撤销 ${plan.rows.filter(r => r.state === "restored").length} 个` : `${readyCount} 个可以处理 · ${(plan?.rows.length ?? 0) - readyCount} 个无需处理或已跳过`}</p>{concentrated && <p className="rename-review-destination"><strong>{plan?.output.mode === "copy" ? "复制到" : "移动到"}</strong><span title={displayPath(plan?.output.directory ?? "")}>{displayPath(plan?.output.directory ?? "")}</span></p>}</div><div className="rename-review-actions"><Button icon={Download} disabled={!!busy || !plan} onClick={() => void exportLog()}>保存日志与汇总</Button>{plan?.journalPath ? <Button icon={RotateCcw} disabled={!!busy || !undoCount} onClick={() => setConfirm("undo")}>{plan?.output.mode === "copy" ? "撤销副本" : concentrated ? "恢复原位" : "恢复原名"}（{undoCount}）</Button> : <Button primary icon={FilePenLine} disabled={!!busy || !readyCount} onClick={() => setConfirm("execute")}>{concentrated ? plan?.output.mode === "copy" ? "重命名并复制" : "重命名并移动" : "执行重命名"}（{readyCount}）</Button>}</div></div><div className="rename-review-filter"><label className="check-label"><input type="checkbox" checked={onlyProblems} onChange={e => { setOnlyProblems(e.target.checked); setPageIndex(0); }} />只看需要处理的问题</label><span>共 {rows.length} 项 · 每页 100 项</span></div><div className="rename-table-scroll"><table><thead><tr><th>原文件名</th><th>{concentrated ? "新文件名 / 目标路径" : "新文件名"}</th><th>状态</th><th>说明 / 校验结果</th></tr></thead><tbody>{rows.slice(pageIndex * 100, (pageIndex + 1) * 100).map(row => <tr key={row.id}><td title={displayPath(row.path)}>{row.originalName}<small>{displayPath(row.path)}</small></td><td>{row.newName || "—"}{row.targetPath && <small title={displayPath(row.targetPath)}>{displayPath(row.targetPath)}</small>}</td><td className={problemStates.has(row.state) ? "rename-invalid" : ""}><span className="rename-state" data-state={row.state}>{stateNames[row.state] ?? row.state}</span>{row.hasMismatch && <span className="rename-check-warning">校验不一致</span>}</td><td>{row.message || "—"}{!!row.checks.length && <details><summary>{row.checks.length} 项校验</summary>{row.checks.map((c, i) => <p key={i}>{c}</p>)}</details>}</td></tr>)}</tbody></table>{!rows.length && <div className="empty-state">没有符合条件的记录。</div>}</div><div className="rename-pagination"><Button disabled={!pageIndex} onClick={() => setPageIndex(v => v - 1)}>上一页</Button><span>{rows.length ? pageIndex + 1 : 0} / {Math.ceil(rows.length / 100)}</span><Button disabled={(pageIndex + 1) * 100 >= rows.length} onClick={() => setPageIndex(v => v + 1)}>下一页</Button></div>{plan?.journalPath && <p className="rename-journal" title={plan.journalPath}>恢复记录已保存在本机：{plan.journalPath}</p>}</main>}
    {closeError && <Modal title="退出前保存失败" onClose={() => { if(!busy) setCloseError(""); }}><div className="modal-body rename-dialog-body"><p>最新的命名规则尚未自动保存，窗口已保留。可重试，或导出加密配置后退出。</p><p className="notice error" role="alert">{closeError}</p></div><div className="modal-footer"><Button disabled={!!busy} onClick={() => setCloseError("")}>返回编辑</Button><Button disabled={!!busy} onClick={() => { setCloseError(""); setExitAfterSave(true); setConfiguration({mode:"save",path:""}); }}>导出配置后退出</Button><Button primary loading={busy === "保存并退出"} disabled={!!busy} onClick={() => void retryClose()}>重试保存并退出</Button></div></Modal>}
    {configuration && <RenameConfigDialog mode={configuration.mode} path={configuration.path} rules={rules} onClose={() => { setConfiguration(null); setExitAfterSave(false); }} onSaved={path => { notify("success","加密配置已导出",fileName(path),path); if(exitAfterSave) void getCurrentWindow().destroy().catch(e => setCloseError(errorText(e))); }} onLoad={preset => { usePreset(preset); notify("success","配置已导入并加入本机",preset.summary.name); }} onWorking={configWorking} />}
    {presetDialog && <RenamePresetDialog key={presetDialog} mode={presetDialog === "list" ? "list" : "save"} current={presetDialog === "new" ? null : currentPreset} rules={rules} onClose={() => setPresetDialog(null)} onSaved={preset => { rememberPreset(preset); notify("success","配置已保存到本机",preset.name); }} onLoad={usePreset} onTransfer={mode => { setPresetDialog(null); if (mode === "save") setConfiguration({mode,path:""}); else void chooseConfiguration(); }} onSaveAs={() => setPresetDialog("new")} onWorking={configWorking} />}
    {lookupSource && <LookupDialog source={lookupSource} onAdd={parts => change({ ...rules, parts: [...rules.parts, ...parts] })} onClose={() => setLookupSource(null)} />}
    {confirm && <Modal title={confirm === "execute" ? concentrated ? "确认集中保存文件" : "确认修改原文件名" : confirm === "undo" ? "确认撤销上次处理" : "清空命名规则"} onClose={() => setConfirm(null)}><p className="modal-body rename-dialog-body">{confirm === "execute" ? concentrated ? `将 ${readyCount} 个文件重命名后${plan?.output.mode === "copy" ? "复制" : "移动"}到：${displayPath(plan?.output.directory ?? "")}。${plan?.output.mode === "copy" ? "原文件保留。" : "成功移动后，原位置不再保留。"}同名文件不会覆盖，异常会记入日志。` : `将在原文件夹中重命名 ${readyCount} 个文件。同名文件不会覆盖，异常会跳过并记入日志；可从“上次记录与恢复”恢复原名。` : confirm === "undo" ? plan?.output.mode === "copy" ? `撤销 ${undoCount} 个生成的副本。仅在原文件仍存在且两边都未变化时移除副本，其他情况保留并说明原因。` : `将 ${undoCount} 个文件恢复到原路径和原名。若文件已变化或原路径被占用，将保留现状并列出原因。` : "将清空命名内容、对照信息和校验规则。磁盘中的文件保持不变。"}</p><div className="modal-footer"><Button onClick={() => setConfirm(null)}>取消</Button><Button primary onClick={() => confirm === "execute" ? void execute() : confirm === "undo" ? void undo() : (change(emptyRules()), setPick(null), setConfirm(null))}>{confirm === "execute" ? concentrated ? plan?.output.mode === "copy" ? "确认复制" : "确认移动" : "确认重命名" : confirm === "undo" ? "确认撤销" : "清空规则"}</Button></div></Modal>}
  </div>;
}
