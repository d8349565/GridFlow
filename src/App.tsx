import {
  useCallback,
  useEffect,
  useReducer,
  useRef,
  useMemo,
  useState,
  lazy,
  Suspense,
} from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  FileSpreadsheet,
  FolderOpen,
  Plus,
  Play,
  Download,
  Settings,
  History,
  SlidersHorizontal,
  ChevronDown,
  ChevronUp,
  Check,
  Pencil,
  Trash2,
  Undo2,
  Redo2,
  MousePointer2,
  Table2,
  BarChart3,
  Layers,
  Save,
  Upload,
  RefreshCw,
  X,
  AlertTriangle,
  LockKeyhole,
  Search,
  ArrowRight,
  ArrowUp,
  ArrowDown,
  Copy,
  Info,
  PanelLeft,
  PanelRight,
  Maximize2,
  Minimize2,
  Heart,
  ExternalLink,
  HelpCircle,
  Sparkles,
  BookOpen,
  Home,
  RotateCcw,
  CheckCircle2,
} from "lucide-react";
import rewardQrCode from "./assets/reward_qr.webp";
import appIcon from "./assets/app_icon.png";
import { Button, Modal, Grid, StaticGrid } from "./components";
import { formatChanges, formatPurpose } from "./review";
import ExcelPreview, { rangeLabel } from "./ExcelPreview";
import SourceFileList from "./SourceFileList";
import OnboardingTour from "./OnboardingTour";
import HelpModal from "./HelpModal";
import {
  call,
  chooseFiles,
  savePath,
  cellText,
  cellDisplay,
  bytes,
  queryFor,
  defaultRules,
  coordinate,
  isTauri,
} from "./api";
import type {
  Workspace,
  RuleSet,
  SourceSheet,
  Column,
  ColumnRule,
  Dataset,
  Task,
  DataPage,
  ImportItem,
  DatasetQuery,
  PivotSpec,
  Analysis,
  Issue,
  Cell,
  Scheme,
  AppError,
  TypeSuggestion,
  SheetRange,
} from "./types";
const Chart = lazy(() => import("./Chart"));
const typeNames = {
  auto: "保留原样（默认）",
  text: "文字 / 编号（保留开头的 0）",
  number: "数字（用于金额、数量）",
  date: "日期（按时间排序）",
  skip: "不加入汇总表",
};
const taskNames = {
  pending: "排队中",
  running: "处理中",
  cancelling: "正在停止",
  cancelled: "已取消",
  succeeded: "已完成",
  failed: "失败",
  interrupted: "已中断",
};
function ruleSignature(rules: RuleSet) {
  return JSON.stringify(rules,(_,value) => value && typeof value === "object" && !Array.isArray(value)
    ? Object.fromEntries(Object.keys(value).sort().map((key) => [key,value[key]])) : value);
}
type Edit = (rules: RuleSet) => RuleSet;
function rulesReducer(
  state: { current: RuleSet; past: RuleSet[]; future: RuleSet[] },
  action:
    | { kind: "edit"; edit: Edit }
    | { kind: "reset"; rules: RuleSet }
    | { kind: "undo" }
    | { kind: "redo" },
) {
  if (action.kind === "reset")
    return { current: action.rules, past: [], future: [] };
  if (action.kind === "undo") {
    if (!state.past.length) return state;
    return {
      current: state.past.at(-1)!,
      past: state.past.slice(0, -1),
      future: [state.current, ...state.future],
    };
  }
  if (action.kind === "redo") {
    if (!state.future.length) return state;
    return {
      current: state.future[0],
      past: [...state.past, state.current],
      future: state.future.slice(1),
    };
  }
  const next = action.edit(structuredClone(state.current));
  if (JSON.stringify(next) === JSON.stringify(state.current)) return state;
  return {
    current: next,
    past: [...state.past.slice(-39), state.current],
    future: [],
  };
}
export default function App() {
  const [ws, setWs] = useState<Workspace | null>(null);
  const [rulesState, dispatch] = useReducer(rulesReducer, {
    current: defaultRules(),
    past: [],
    future: [],
  });
  const rules = rulesState.current;
  const [activeSheet, setActiveSheet] = useState<string | null>(null);
  const [tab, setTab] = useState<
    "source" | "preview" | "result" | "pivot" | "chart"
  >("source");
  const [phase,setPhase] = useState<"configure" | "results">("configure");
  const [homeView, setHomeView] = useState(false);
  const [panels, setPanels] = useState({ files: true, rules: true });
  const previousPanels = useRef(panels);
  const phaseRef = useRef(phase); phaseRef.current=phase;
  const tabRef = useRef(tab); tabRef.current=tab;
  const handledTaskIds = useRef<Set<string>>(new Set());
  const [panelTab, setPanelTab] = useState("sheets");
  const [busy, setBusy] = useState("");
  const busyRef = useRef(false);
  const [notice, setNotice] = useState<{
    message: string;
    error: boolean;
  } | null>(null);
  const [modal, setModal] = useState<string | null>(null);
  const [fileSidebarWidth, setFileSidebarWidth] = useState<number>(() => {
    try {
      const saved = Number(localStorage.getItem("gridflow_file_sidebar_width"));
      return saved >= 180 && saved <= 600 ? saved : 216;
    } catch {
      return 216;
    }
  });
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [settingsTab, setSettingsTab] = useState<"general" | "about">("general");

  const handleResizerPointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      e.preventDefault();
      const target = e.currentTarget;
      try {
        target.setPointerCapture(e.pointerId);
      } catch {}
      setIsResizingSidebar(true);
      const startX = e.clientX;
      const startWidth = fileSidebarWidth;

      const onPointerMove = (ev: PointerEvent) => {
        const delta = ev.clientX - startX;
        const nextWidth = Math.min(600, Math.max(180, Math.round(startWidth + delta)));
        setFileSidebarWidth(nextWidth);
      };
      const onPointerUp = (ev: PointerEvent) => {
        try {
          target.releasePointerCapture(ev.pointerId);
        } catch {}
        setIsResizingSidebar(false);
        window.removeEventListener("pointermove", onPointerMove);
        window.removeEventListener("pointerup", onPointerUp);
        setFileSidebarWidth((w) => {
          try {
            localStorage.setItem("gridflow_file_sidebar_width", String(w));
          } catch {}
          return w;
        });
      };
      window.addEventListener("pointermove", onPointerMove);
      window.addEventListener("pointerup", onPointerUp);
    },
    [fileSidebarWidth],
  );
  const [columns, setColumns] = useState<Column[]>([]);
  const [tourActive, setTourActive] = useState(false);
  const [tourStep, setTourStep] = useState(0);
  const [inputColumns, setInputColumns] = useState<Column[]>([]);
  const [typeHints, setTypeHints] = useState<TypeSuggestion[]>([]);
  const [hintsLoading, setHintsLoading] = useState(false);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [reviewAttempt, setReviewAttempt] = useState(0);
  const [focusRow, setFocusRow] = useState<number>();
  const [customValues, setCustomValues] = useState<string[]>([]);
  const [rawPage, setRawPage] = useState<DataPage | null>(null);
  const [preview, setPreview] = useState<Dataset | null>(null);
  const [resultId, setResultId] = useState<string | null>(null);
  const [requestedTaskId,setRequestedTaskId] = useState<string | null>(null);
  const [selections,setSelections] = useState<Record<string,SheetRange>>({});
  const [selectedCell,setSelectedCell] = useState<Cell | null>(null);
  const [selectionError,setSelectionError] = useState("");
  const [headerCandidate,setHeaderCandidate] = useState<{ range: SheetRange; columns: Column[] } | null>(null);
  const [headerScope,setHeaderScope] = useState(true);
  const [fixedCandidate, setFixedCandidate] = useState<{
    row: number;
    column: number;
    value: Cell;
    sheet: SourceSheet;
  } | null>(null);
  const [query, setQuery] = useState<DatasetQuery>(queryFor(""));
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
  const [chartType, setChartType] = useState("bar");
  const chartPng = useRef<(() => string) | null>(null);
  const [pivot, setPivot] = useState<PivotSpec>({
    query: queryFor(""),
    rowFields: [],
    columnFields: [],
    valueFields: [],
    aggregation: "sum",
    showRowTotals: true,
    sort: null,
    customOrder: [],
  });
  const [importReport, setImportReport] = useState<ImportItem[]>([]);
  const [selectedResults, setSelectedResults] = useState<string[]>([]);
  const [schemeId, setSchemeId] = useState("");
  const [issues, setIssues] = useState<Issue[]>([]);
  const [issueOffset, setIssueOffset] = useState(0);
  const [confirm, setConfirm] = useState<{
    message: string;
    action: () => Promise<void>;
  } | null>(null);
  const [disk, setDisk] = useState(0);
  const [exportFormat, setExportFormat] = useState("xlsx");
  const [filterKey, setFilterKey] = useState("");
  const [filterValues, setFilterValues] = useState<string[]>([]);
  const [chosenValues, setChosenValues] = useState<string[]>([]);
  const [filterOffset, setFilterOffset] = useState(0);
  const [filterSearch, setFilterSearch] = useState("");
  const [saveName, setSaveName] = useState("");
  const [renameName, setRenameName] = useState("");
  const [password, setPassword] = useState("");
  const [currentPassword, setCurrentPassword] = useState("");
  const [selectedHintKeys, setSelectedHintKeys] = useState<string[]>([]);
  const [suggestHeaders, setSuggestHeaders] = useState(false);
  const ready = useRef(false);
  const gridRequest = useRef(0);
  const latestRules = useRef(rules);
  latestRules.current = rules;
  const latestActive = useRef(activeSheet);
  latestActive.current = activeSheet;
  const latestLocked = useRef(ws?.locked);
  latestLocked.current = ws?.locked;
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const window = getCurrentWindow();
    window
      .onCloseRequested(async (event) => {
        event.preventDefault();
        try {
          if (ready.current && !latestLocked.current)
            await call("save_draft", {
              rules: latestRules.current,
              activeSheetId: latestActive.current,
            });
        } catch (e) {
          console.error("Failed to save draft on close:", e);
        } finally {
          await window.destroy();
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
  const edit = useCallback(
    (edit: Edit) => dispatch({ kind: "edit", edit }),
    [],
  );
  const running = ws?.tasks.find((t) =>
    ["running", "pending", "cancelling"].includes(t.state),
  );
  const sheet = ws?.files
    .flatMap((f) => f.sheets)
    .find((s) => s.id === activeSheet);
  const source = rules.sources.find((s) => s.sheetId === activeSheet);
  const dataset =
    tab === "preview"
      ? preview
      : (ws?.datasets.find((d) => d.id === resultId) ?? (!running ? ws?.datasets[0] : undefined) ?? null);
  const signature = useMemo(() => ruleSignature(rules),[rules]);
  const staleResult = !!dataset && signature !== ruleSignature(dataset.rules);
  const currentHeader = source?.headerRange ?? { startRow: source?.headerRow ?? 1,endRow: source?.headerRow ?? 1,startColumn: 1,endColumn: sheet?.width ?? 1 };
  const selection = selections[activeSheet ?? ""] ?? null;
  const singleSelection = !!selection && selection.startRow===selection.endRow && selection.startColumn===selection.endColumn;
  const requestedTask=ws?.tasks.find((t) => t.id===requestedTaskId);
  const stoppedTask=requestedTask && ["failed","cancelled","interrupted"].includes(requestedTask.state) ? requestedTask : null;
  const selectedSheets = rules.sources.filter((s) => s.enabled).length;
  const selectedFiles = ws?.files.filter((f) => f.sheets.some((s) =>
    rules.sources.some((r) => r.sheetId === s.id && r.enabled),
  )) ?? [];
  const isHome = homeView || (!ws?.files.length && phase === "configure");
  const selectedItems = selectedFiles.flatMap((f) => f.sheets.flatMap((s) => {
    const selection = rules.sources.find((r) => r.sheetId === s.id && r.enabled);
    return selection ? [{ file: f, sheet: s, source: selection }] : [];
  }));
  const estimatedRows = selectedItems.reduce((sum, item) =>
    sum + Math.max(0, item.sheet.rows - item.source.headerRow), 0,
  );
  const headerChanges = selectedItems.filter((item) =>
    !item.source.headerRange && item.sheet.suggestedHeaderRow !== item.source.headerRow,
  );
  const headerGroups = [...new Set(headerChanges.map((item) => JSON.stringify([
    item.sheet.name, item.source.headerRow, item.sheet.suggestedHeaderRow,
  ])))].map((key) => {
    const [name, current, suggested] = JSON.parse(key) as [string, number, number];
    return { name, current, suggested, items: headerChanges.filter((item) =>
      item.sheet.name === name && item.source.headerRow === current && item.sheet.suggestedHeaderRow === suggested,
    ) };
  });
  const usefulHints = formatChanges(typeHints, rules);
  const chosenHints = usefulHints.filter((h) => selectedHintKeys.includes(h.key));
  const hasReviewSelection = (suggestHeaders && headerChanges.length > 0) || chosenHints.length > 0;
  const mergedColumns = columns.map(
    (c) =>
      rules.columns.find((r) => r.key === c.key) ??
      ({
        key: c.key,
        name: c.name,
        dataType: "auto",
        hidden: false,
      } as ColumnRule),
  );
  const allColumns = [
    ...rules.columns.filter((r) => columns.some((c) => c.key === r.key)),
    ...mergedColumns.filter((c) => !rules.columns.some((r) => r.key === c.key)),
  ];

  function error(error: unknown) {
    const message =
      typeof error === "object" && error !== null && "message" in error
        ? String((error as AppError).message)
        : String(error);
    setNotice({ message, error: true });
  }
  async function action<T>(
    name: string,
    work: () => Promise<T>,
  ): Promise<T | undefined> {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(name);
    setNotice(null);
    try {
      return await work();
    } catch (e) {
      error(e);
    } finally {
      busyRef.current = false;
      setBusy("");
    }
  }
  async function refresh(initial = false) {
    const value = await call<Workspace>("workspace");
    setWs(value);
    if (initial) {
      dispatch({ kind: "reset", rules: value.rules });
      setActiveSheet(value.activeSheetId);
      setResultId(value.datasets[0]?.id ?? null);
      setSchemeId(value.schemes.at(-1)?.id ?? "");
      ready.current = !value.locked;
      for (const t of value.tasks) {
        if (!["pending", "running", "cancelling"].includes(t.state)) {
          handledTaskIds.current.add(t.id);
        }
      }
    }
    return value;
  }
  useEffect(() => {
    refresh(true).catch(error);
  }, []);
  useEffect(() => {
    if (!running && ws?.datasets.length) {
      if (!resultId || !ws.datasets.some((d) => d.id === resultId)) {
        const nextId = ws.datasets[0].id;
        setResultId(nextId);
        setQuery(queryFor(nextId));
      }
    }
  }, [ws?.datasets, resultId, running]);
  useEffect(() => {
    if (!notice || notice.error || modal) return;
    const timer = setTimeout(() => setNotice(null), 8000);
    return () => clearTimeout(timer);
  }, [notice, modal]);
  useEffect(() => {
    if (!ready.current || ws?.locked) return;
    const timer = setTimeout(
      () =>
        call("save_draft", { rules, activeSheetId: activeSheet }).catch(error),
      450,
    );
    return () => clearTimeout(timer);
  }, [rules, activeSheet, ws?.locked]);
  useEffect(() => {
    if (!ws || ws.locked || !activeSheet) return;
    setRawPage(null);
    const revision = ++gridRequest.current;
    call<DataPage>("preview_sheet", {
      sheetId: activeSheet,
      offset: 0,
      limit: 100,
      headerRow: null,
    })
      .then((page) => {
        if (revision === gridRequest.current) setRawPage(page);
      })
      .catch(error);
  }, [activeSheet, ws?.files.length, ws?.locked]);
  const sourceSignature = JSON.stringify([
    rules.sources,
    rules.mappings,
    rules.fixedCells,
  ]);
  useEffect(() => {
    if (!ready.current || ws?.locked || !selectedSheets) {
      setColumns([]);
      return;
    }
    let cancelled = false;
    call<Column[]>("available_columns", { rules })
      .then((cols) => {
        if (!cancelled) setColumns(cols);
      })
      .catch(error);
    return () => {
      cancelled = true;
    };
  }, [sourceSignature, ws?.locked]);
  useEffect(() => {
    if (!ready.current || ws?.locked || !selectedSheets) {
      setInputColumns([]);
      return;
    }
    let cancelled = false;
    call<Column[]>("input_columns", { rules })
      .then((c) => {
        if (!cancelled) setInputColumns(c);
      })
      .catch(error);
    return () => {
      cancelled = true;
    };
  }, [sourceSignature, ws?.locked]);
  useEffect(() => {
    if (modal !== "suggest") return;
    let cancelled = false;
    setHintsLoading(true);
    setReviewError(null);
    const proposed = {
      ...rules,
      sources: rules.sources.map((source) => {
        const found = ws?.files
          .flatMap((f) => f.sheets)
          .find((s) => s.id === source.sheetId);
        return suggestHeaders && found && source.enabled && !source.headerRange
          ? { ...source, headerRow: found.suggestedHeaderRow }
          : source;
      }),
    };
    call<TypeSuggestion[]>("suggestions", { rules: proposed })
      .then((hints) => {
        if (!cancelled) setTypeHints(hints);
      })
      .catch((e) => { if (!cancelled) setReviewError(e.message ?? String(e)); })
      .finally(() => {
        if (!cancelled) setHintsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [modal, suggestHeaders, sourceSignature, reviewAttempt]);
  useEffect(() => {
    if (!ws || ws.locked) return;
    let disposed = false;
    const tick = async () => {
      try {
        const tasks = await call<Task[]>("tasks");
        if (disposed) return;
        const newlyDone = tasks.find(
          (t) =>
            !handledTaskIds.current.has(t.id) &&
            t.state === "succeeded" &&
            Boolean(t.datasetId),
        );
        tasks.forEach((t) => {
          if (!["pending", "running", "cancelling"].includes(t.state)) {
            handledTaskIds.current.add(t.id);
          }
        });
        if (newlyDone) {
          const value = await call<Workspace>("workspace");
          if (disposed) return;
          const nextDataset =
            value.datasets.find((d) => d.id === newlyDone.datasetId) ??
            value.datasets[0];
          if (nextDataset) {
            setResultId(nextDataset.id);
            setQuery(queryFor(nextDataset.id));
          }
          if (phaseRef.current === "results" && tabRef.current === "result") {
            setTab("result");
          }
          setNotice({
            message: "汇总表已生成。点击「保存到电脑」就能拿到文件，也可以先筛选或做分类统计。",
            error: false,
          });
          setWs(value);
        } else {
          setWs((old) => (old ? { ...old, tasks } : old));
        }
      } catch (e) {
        if (!disposed) error(e);
      }
    };
    const timer = setInterval(tick, running ? 750 : 4000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [ws?.locked, ws?.datasets.length, !!running]);
  useEffect(() => {
    if (!isTauri()) return;
    let stop: (() => void) | undefined;
    let disposed = false;
    getCurrentWebviewWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop")
          void importPaths(event.payload.paths);
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, []);
  useEffect(() => {
    function key(event: KeyboardEvent) {
      const target = event.target as HTMLElement;
      const editing = target.matches("input,textarea,[contenteditable=true]");
      if (event.ctrlKey && event.key === "o") {
        event.preventDefault();
        void importChoose();
      }
      if (event.ctrlKey && event.key === "Enter") {
        event.preventDefault();
        void generate();
      }
      if (!editing && event.ctrlKey && event.key === "z") {
        event.preventDefault();
        dispatch({ kind: event.shiftKey ? "redo" : "undo" });
      }
      if (!editing && event.ctrlKey && event.key === "y") {
        event.preventDefault();
        dispatch({ kind: "redo" });
      }
    }
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, [rules, selectedSheets]);
  async function importChoose(directory = false, append = true) {
    const paths = await chooseFiles(directory);
    if (paths.length) await importPaths(paths, append);
  }
  async function importPaths(paths: string[], append = true) {
    await action("导入文件", async () => {
      const report = await call<ImportItem[]>("import_files", { paths });
      setImportReport(report);
      const value = await refresh();
      const added = new Set(
        report.flatMap((item) => item.file?.sheets.map((s) => s.id) ?? []),
      );
      const next = {
        ...value.rules,
        sources: value.rules.sources.map((s) => ({
          ...s,
          enabled: added.has(s.sheetId) || (append && s.enabled),
        })),
      };
      dispatch({ kind: "edit", edit: () => next });
      setActiveSheet([...added][0] ?? value.activeSheetId);
      setTab("source");
      setPhase("configure");  setSelectedCell(null); setSelectionError("");
      setHomeView(false);
      setPreview(null);
      if (report.some((item) => item.error)) setModal("imports");
      else {
        setModal(null);
        setNotice({ message: `已添加 ${report.length} 个文件，并勾选其中的工作表。`, error: false });
        if (!localStorage.getItem("gridflow_tour_completed")) {
          setTourStep(0);
          setTourActive(true);
        }
      }
    });
  }
  async function loadSample() {
    await action("载入示例数据", async () => {
      const report = await call<ImportItem[]>("load_sample_files");
      setImportReport(report);
      const value = await refresh();
      const added = new Set(
        report.flatMap((item) => item.file?.sheets.map((s) => s.id) ?? []),
      );
      const next = {
        ...value.rules,
        sources: value.rules.sources.map((s) => ({
          ...s,
          enabled: added.has(s.sheetId),
        })),
      };
      dispatch({ kind: "edit", edit: () => next });
      setActiveSheet([...added][0] ?? value.activeSheetId);
      setTab("source");
      setPhase("configure");
      setHomeView(false);
      setSelectedCell(null);
      setSelectionError("");
      setPreview(null);
      setPanels({ files: true, rules: true });
      setNotice({
        message: "已载入 2 份示例连锁门店销售月报，已为你开启实操引导！",
        error: false,
      });
      setTourStep(0);
      setTourActive(true);
    });
  }
  async function generate(nextRules = latestRules.current) {
    await action("提交任务", async () => {
      const task = await call<Task>("submit_task", {
        rules: nextRules,
      });
      setRequestedTaskId(task.id);
      setPhase("results"); setTab("result"); setResultId(null);
      setWs((old) => (old ? { ...old, tasks: [task, ...old.tasks] } : old));
      setNotice({
        message: "正在合并所选表格，完成后会显示总计表。你可以继续查看文件。",
        error: false,
      });
    });
  }
  async function previewRules() {
    await action("规则预览", async () => {
      const ds = await call<Dataset>("preview_rules", { rules });
      setPreview(ds);
      setQuery(queryFor(ds.id));
      setTab("preview");
      setPhase("results");
    });
  }
  async function applyScheme() {
    if (!schemeId) return;
    const applied = await action("应用方案", async () => {
      const next = await call<RuleSet>("apply_scheme", { schemeId, rules });
      edit(() => next);
      setPreview(null);
      setNotice({
        message: "已使用保存的设置。点击「配置完成，预览结果」即可继续。",
        error: false,
      });
      return true;
    });
    if (applied) setModal(null);
  }
  async function saveScheme() {
    await action("保存方案", async () => {
      const scheme = await call<Scheme>("save_scheme", {
        name: saveName,
        rules,
      });
      setSchemeId(scheme.id);
      setSaveName("");
      await refresh();
      setModal(null);
      setNotice({
        message: `已保存「${scheme.name}」。下次添加同类文件后，可在「配置文件」中选它。`,
        error: false,
      });
    });
  }
  async function renameScheme() {
    if (!schemeId) return;
    await action("重命名方案", async () => {
      const scheme = await call<Scheme>("rename_scheme", {
        schemeId,
        name: renameName,
      });
      setRenameName("");
      await refresh();
      setNotice({
        message: `已重命名为「${scheme.name}」。`,
        error: false,
      });
    });
  }
  function updateColumn(key: string, patch: Partial<ColumnRule>) {
    edit((r) => {
      r.columns = allColumns.map((c) =>
        c.key === key ? { ...c, ...patch } : c,
      );
      return r;
    });
  }
  function moveColumn(key: string, direction: number) {
    const i = allColumns.findIndex((c) => c.key === key);
    const j = i + direction;
    if (j < 0 || j >= allColumns.length) return;
    edit((r) => {
      const next = [...allColumns];
      [next[i], next[j]] = [next[j], next[i]];
      r.columns = next;
      return r;
    });
  }
  function selectArea(range: SheetRange,cell: Cell | null) {
    if (!activeSheet) return;
    setSelections((old) => ({ ...old,[activeSheet]:range })); setSelectedCell(cell); setSelectionError("");
  }
  async function reviewHeader() {
    if (!selection || !sheet) return;
    if (selection.endRow-selection.startRow >= 32) { setSelectionError("表头最多支持 32 行，请缩小选区。"); return; }
    const area=selection;
    await action("检查表头",async () => {
      const proposed={ ...rules,sources:rules.sources.map((s) => ({ ...s,enabled:s.sheetId===sheet.id,headerRow:s.sheetId===sheet.id?area.endRow:s.headerRow,headerRange:s.sheetId===sheet.id?area:s.headerRange })) };
      const cols=await call<Column[]>("input_columns",{ rules:proposed });
      setHeaderCandidate({ range:area,columns:cols });setHeaderScope(true);setModal("header");
    });
  }
  function applyHeader() {
    if (!headerCandidate || !sheet) return;
    const area=headerCandidate.range;
    const targets=selectedItems.filter((item) => item.sheet.id===sheet.id || (headerScope && item.sheet.name===sheet.name));
    const bad=targets.filter((item) => item.sheet.rows<area.endRow || item.sheet.width<area.endColumn);
    if (bad.length) { setSelectionError(`选区超出 ${bad[0].file.name} 的范围。可以取消批量应用，单独设置此表。`); return; }
    const ids=new Set(targets.map((item) => item.sheet.id)); ids.add(sheet.id);
    edit((r) => ({ ...r,sources:r.sources.map((s) => ids.has(s.sheetId)?{ ...s,headerRow:area.endRow,headerRange:area }:s) }));
    setModal(null);setSelectionError("");
    setNotice({ message:`已设置 ${targets.length || 1} 张表的表头 ${rangeLabel(area)}，从第 ${area.endRow+1} 行开始读取。`,error:false });
  }
  function addSelectedCell() {
    if (!singleSelection || !selection || !selectedCell || !sheet) return;
    setFixedCandidate({ row:selection.startRow,column:selection.startColumn,value:selectedCell,sheet });
    const left=rawPage?.rows[selection.startRow-1]?.[selection.startColumn-2];
    const label=cellText(left).replace(/[：:]\s*$/,"");
    setSaveName(label && label.length<=20 && /名称|门店|机构|日期|部门|负责人|区域|编号/.test(label)?label:`附加信息 ${rules.fixedCells.length+1}`);
    setModal("fixed");
  }
  function goHome() {
    setModal(null);
    setTourActive(false);
    if (isHome && ws?.files.length) {
      confirmResetToInitial();
    } else {
      setHomeView(true);
    }
  }
  function confirmResetToInitial() {
    const fileCount = ws?.files.length ?? 0;
    setConfirm({
      message: `确认清空当前导入的 ${fileCount} 个文件并恢复初始页面状态？工作台内部记录会被清理，删除后无法恢复。`,
      action: async () => {
        const fileIds = (ws?.files ?? []).map((f) => f.id);
        if (fileIds.length > 0) {
          await call("delete_items", { kind: "files", ids: fileIds });
        }
        const value = await refresh();
        dispatch({ kind: "reset", rules: value.rules });
        setActiveSheet(null);
        setRawPage(null);
        setHomeView(false);
        setTourActive(false);
        setPhase("configure");
        setConfirm(null);
      },
    });
  }
  function returnToConfig() { setPhase("configure");setTab("source");setHomeView(false); }
  function openReview() {
    setSuggestHeaders(false);
    setSelectedHintKeys([]);
    setReviewAttempt((n) => n + 1);
    setModal("suggest");
  }
  function applySuggestions(merge = false) {
    const next = structuredClone(rules);
    const r = next;
      if (suggestHeaders)
        r.sources = r.sources.map((s) => {
          const sheet = ws?.files
            .flatMap((f) => f.sheets)
            .find((x) => x.id === s.sheetId);
          return sheet && s.enabled && !s.headerRange
            ? { ...s, headerRow: sheet.suggestedHeaderRow }
            : s;
        });
      if (hasReviewSelection) {
        const hinted = suggestHeaders ? typeHints.map((hint) => {
          const existing = r.columns.find((c) => c.key === hint.key);
          return {
            key: hint.key,
            name: existing?.name ?? hint.name,
            hidden: existing?.hidden ?? false,
            dataType: existing?.dataType ?? "auto",
          } as ColumnRule;
        }) : allColumns;
        const extras = allColumns.filter(
          (c) => c.key === "__source" || c.key.startsWith("fixed:"),
        );
        r.columns = [...hinted, ...extras.filter((c) => !hinted.some((h) => h.key === c.key))]
          .map((c) => {
            const hint = chosenHints.find((h) => h.key === c.key);
            return hint ? { ...c, dataType: hint.suggestedType } : c;
          });
      }
    edit(() => next);
    setModal(null);
    setNotice({
      message: "已使用所选调整。可以查看合并预览，或直接合并全部数据。",
      error: false,
    });
    if (merge) void generate(next);
  }
  async function exportCurrent(
    kind: "dataset" | "pivot" | "image" = "dataset",
  ) {
    if (!dataset) return;
    await action("导出", async () => {
      if (kind === "image") {
        if (!chartPng.current) return;
        const path = await savePath(
          "分析图表.png",
          "png",
          ws?.settings.lastExportDirectory,
        );
        if (!path) return;
        const binary = atob(chartPng.current().split(",")[1]);
        await call("save_image", {
          path,
          bytes: Array.from(binary, (c) => c.charCodeAt(0)),
        });
        setNotice({ message: `图表已保存：${path}`, error: false });
        return;
      }
      const filename = `${kind === "pivot" ? "分类统计" : "汇总表"}_${new Date().toLocaleDateString("sv-SE")}.${exportFormat}`;
      const path = await savePath(
        filename,
        exportFormat,
        ws?.settings.lastExportDirectory,
      );
      if (!path) return;
      await call(
        kind === "pivot" ? "export_analysis" : "export_dataset",
        kind === "pivot"
          ? {
              spec: analysis?.spec ?? {
                ...pivot,
                query: { ...query, datasetId: dataset.id },
              },
              path,
            }
          : { query: { ...query, datasetId: dataset.id }, path },
      );
      await refresh();
      setNotice({ message: `已保存：${path}`, error: false });
    });
  }
  async function showIssues(ds: Dataset, offset = 0) {
    await action("读取异常", async () => {
      setIssues(await call<Issue[]>("issues", { datasetId: ds.id, offset }));
      setIssueOffset(offset);
      setModal("issues");
    });
  }
  async function loadValues(key: string, offset = 0) {
    if (!dataset) return;
    await action("读取筛选值", async () => {
      const values = await call<string[]>("distinct_values", {
        query: {
          ...query,
          datasetId: dataset.id,
          filters: query.filters.filter((f) => f.key !== key),
        },
        key,
        offset,
      });
      setFilterKey(key);
      setFilterValues((old) => (offset ? [...old, ...values] : values));
      setFilterOffset(offset);
      setFilterSearch("");
      if (!offset)
        setChosenValues(query.filters.find((f) => f.key === key)?.values ?? []);
      setModal("filter");
    });
  }
  async function generateAnalysis() {
    if (!dataset) return;
    await action("计算分析", async () => {
      const spec = { ...pivot, query: { ...query, datasetId: dataset.id } };
      const result = await call<Analysis>("aggregate", { spec });
      setAnalysis(result);
    });
  }
  function addField(
    group: "rowFields" | "columnFields" | "valueFields",
    key: string,
  ) {
    setPivot((p) => {
      const next = {
        ...p,
        [group]: p[group].includes(key)
          ? p[group].filter((v) => v !== key)
          : [...p[group], key],
        sort: null,
      };
      if (group === "rowFields")
        next.columnFields = next.columnFields.filter((v) => v !== key);
      if (group === "columnFields")
        next.rowFields = next.rowFields.filter((v) => v !== key);
      return next;
    });
    setAnalysis(null);
  }
  async function batchExport(zip = false) {
    await action("批量导出", async () => {
      if (!selectedResults.length) return;
      const dirs = await chooseFiles(true);
      if (!dirs.length) return;
      const zipPath = zip
        ? await savePath("汇总结果合集.zip", "zip", dirs[0])
        : null;
      if (zip && !zipPath) return;
      const paths = await call<string[]>("export_batch", {
        ids: selectedResults,
        directory: dirs[0],
        zipPath,
        format: exportFormat,
      });
      await refresh();
      setNotice({
        message: `已保存 ${paths.length - (zip ? 1 : 0)} 个结果${zip ? "及 ZIP 合集" : ""}。`,
        error: false,
      });
    });
  }
  function askDelete(kind: string, ids: string[]) {
    setConfirm({
      message: `确认删除 ${ids.length} 个${kind === "files" ? "导入文件副本" : kind === "schemes" ? "保存方案" : "处理结果"}？工作台内部记录会被清理，删除后无法恢复。`,
      action: async () => {
        await call("delete_items", { kind, ids });
        const value = await refresh();
        if (kind === "files") {
          dispatch({ kind: "reset", rules: value.rules });
          setActiveSheet(value.activeSheetId);
          setRawPage(null);
        }
        if (ids.includes(resultId ?? "")) {
          const remaining = value.datasets.find((d) => !ids.includes(d.id));
          setResultId(remaining?.id ?? null);
          if (remaining) setQuery(queryFor(remaining.id));
        }
        setSelectedResults((old) => old.filter((id) => !ids.includes(id)));
        if (ids.includes(schemeId)) setSchemeId("");
        setConfirm(null);
      },
    });
  }
  const noticeBox = notice ? (
    <div
      className={notice.error ? "notice error" : "notice success"}
      role={notice.error ? "alert" : "status"}
    >
      <span>{notice.message}</span>
      <button aria-label="关闭提示" onClick={() => setNotice(null)}>
        <X size={15} />
      </button>
    </div>
  ) : null;
  if (ws?.locked)
    return (
      <div className="lock-screen">
        <div className="lock-card">
          <div className="app-mark">
            <FileSpreadsheet size={26} />
          </div>
          <h1>解锁工作台</h1>
          <p>输入启动密码，继续处理你的文件。</p>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void action("解锁", async () => {
                await call("unlock", { password });
                setPassword("");
                await refresh(true);
              });
            }}
          >
            <label>
              启动密码
              <input
                autoFocus
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </label>
            {noticeBox}
            <Button primary loading={!!busy} type="submit" icon={LockKeyhole}>
              解锁
            </Button>
          </form>
          <small>文件和处理结果保存在本机。</small>
        </div>
      </div>
    );
  return (
    <div className="app-shell">
      <header className="app-header">
        <div
          className="brand"
          role="button"
          tabIndex={0}
          title="回到主页（初始页面状态）"
          onClick={goHome}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") goHome();
          }}
        >
          <div className="app-mark">
            <FileSpreadsheet size={18} />
          </div>
          <strong>汇格 · GridFlow</strong>
        </div>
        {!!(ws?.files.length || ws?.datasets.length) && <nav className="stage-nav" aria-label="汇总步骤">
          <button className={phase === "configure" && !isHome ? "active" : ""} aria-current={phase === "configure" && !isHome ? "step" : undefined} aria-label={phase === "results" ? "返回修改配置" : undefined} title="配置表头、输出列与整理方式" onClick={returnToConfig}><span aria-hidden="true">1</span>配置汇总内容</button>
          <button className={phase === "results" && !isHome ? "active" : ""} aria-current={phase === "results" && !isHome ? "step" : undefined} disabled={!preview && !ws?.datasets.length && !running} onClick={() => { setHomeView(false); setPhase("results");setTab(preview ? "preview" : "result");if(preview)setQuery(queryFor(preview.id)); }}><span aria-hidden="true">2</span>预览与保存</button>
        </nav>}
        <div className="header-actions">
          {phase === "configure" && !!ws?.files.length && !isHome && <>
            <div className="edit-actions">
              <Button className="quiet icon-button" icon={Undo2} aria-label="撤销配置修改" title="撤销上一次调整" disabled={!rulesState.past.length} onClick={() => dispatch({ kind:"undo" })} />
              <Button className="quiet icon-button" icon={Redo2} aria-label="重做配置修改" title="恢复撤销的调整" disabled={!rulesState.future.length} onClick={() => dispatch({ kind:"redo" })} />
            </div>
            <Button className="quiet" icon={Save} disabled={!selectedSheets} title="保存或套用汇总配置文件" onClick={() => setModal("reuse")}>配置文件</Button>
          </>}
          <Button
            className={`quiet icon-button ${isHome ? "selected" : ""}`}
            icon={Home}
            aria-label="回到主页"
            title={isHome && ws?.files.length ? "已在主页（点击可清空并恢复初始状态）" : "回到主页（初始页面状态）"}
            onClick={goHome}
          />
          <Button className="quiet history-button" icon={History} onClick={() => setModal("history")}>
            历史结果{ws?.datasets.length ? <span className="count">{ws.datasets.length}</span> : null}
          </Button>
          <Button className="quiet icon-button" icon={HelpCircle} aria-label="帮助与指引" title="帮助与新手指引" onClick={() => setModal("help")} />
          <Button className="quiet icon-button" icon={Settings} aria-label="设置" title="设置" onClick={() => {
            setModal("settings");void call<number>("disk_usage").then(setDisk).catch(error);
          }} />
          {phase === "configure" && !!ws?.files.length && !isHome ? <Button data-tour="preview-btn" icon={Play} primary disabled={!selectedSheets || !!busy} loading={busy === "提交任务"} onClick={() => void generate()}>预览结果</Button>
            : phase === "results" && !isHome && tab !== "pivot" && tab !== "chart" ? <>
              {staleResult && <Button icon={RefreshCw} primary disabled={!!busy || !selectedSheets} onClick={() => void generate()}>按当前配置刷新结果</Button>}
              <select aria-label="导出格式" value={exportFormat} onChange={(e) => setExportFormat(e.target.value)}><option value="xlsx">Excel</option><option value="csv">CSV</option></select>
              <Button icon={Download} primary={!staleResult} disabled={!dataset || !!busy} loading={busy === "导出"} onClick={() => void exportCurrent()}>{staleResult ? "保存此版本" : query.filters.length || query.search ? "保存筛选后的表格" : "保存到电脑"}</Button>
            </> : null}
        </div>
      </header>
      {notice && !modal && <div className={`workspace-notice ${notice.error ? "persistent" : ""}`}>{noticeBox}</div>}
      <div
        className={`workspace ${isResizingSidebar ? "is-resizing" : ""} ${isHome ? "files-hidden rules-hidden" : phase === "results" ? "results-workspace" : `${!ws?.files.length || !panels.files ? "files-hidden" : ""} ${!ws?.files.length || !panels.rules ? "rules-hidden" : ""}`}`}
        style={{ "--file-width": `${fileSidebarWidth}px` } as React.CSSProperties}
      >
        {phase === "configure" && !isHome && !!ws?.files.length && panels.files && <aside className="file-sidebar" data-tour="file-sidebar" tabIndex={0} onKeyDown={(e) => {
          if (e.key === "Delete" && selectedFiles.length > 0 && !modal && !confirm) {
            askDelete("files", selectedFiles.map((f) => f.id));
          }
        }}>
          <div className="section-title">
            <label className="source-all">
              <input type="checkbox" aria-label="全选工作表" title="全选或取消全部工作表"
                checked={rules.sources.length > 0 && rules.sources.every((source) => source.enabled)}
                ref={(input) => { if (input) input.indeterminate = selectedSheets > 0 && selectedSheets < rules.sources.length; }}
                onChange={(event) => {
                  const enabled = event.target.checked;
                  edit((r) => ({
                    ...r,
                    sources: r.sources.map((s) => ({ ...s, enabled })),
                  }));
              }} />
              <span>文件 <b>{ws?.files.length ?? 0}</b></span>
            </label>
            {selectedFiles.length > 0 && (
              <button
                type="button"
                className="file-batch-delete"
                title={selectedFiles.length === (ws?.files.length ?? 0) ? `删除全部 ${selectedFiles.length} 个文件` : `批量删除选中的 ${selectedFiles.length} 个文件`}
                aria-label={`批量删除选中的 ${selectedFiles.length} 个文件`}
                onClick={() => askDelete("files", selectedFiles.map((f) => f.id))}
              >
                <Trash2 size={13} />
                <span>{selectedFiles.length === (ws?.files.length ?? 0) ? `全部删除 (${selectedFiles.length})` : selectedFiles.length > 1 ? `批量删除 (${selectedFiles.length})` : `删除已选 (${selectedFiles.length})`}</span>
              </button>
            )}
          </div>
          <div className="file-actions">
            <Button icon={Plus} loading={busy === "导入文件"} disabled={!!busy} onClick={() => void importChoose()}>添加文件</Button>
            <details className="import-menu" onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) e.currentTarget.open = false; }} onKeyDown={(e) => { if (e.key === "Escape") { e.currentTarget.open = false;e.currentTarget.querySelector("summary")?.focus(); } }}>
              <summary aria-label="更多导入方式" title="添加文件夹或换一批文件"><ChevronDown size={15} /></summary>
              <div className="import-options">
                <Button icon={FolderOpen} disabled={!!busy} onClick={(e) => { e.currentTarget.closest("details")!.open = false;void importChoose(true, true); }}>添加文件夹</Button>
                <Button icon={RefreshCw} disabled={!!busy} title="更换本次文件，之前的文件和结果仍保留" onClick={(e) => { e.currentTarget.closest("details")!.open = false;void importChoose(false, false); }}>换一批文件</Button>
              </div>
            </details>
          </div>
          <SourceFileList files={ws?.files ?? []} sources={rules.sources} activeSheet={activeSheet}
            onSelect={(ids, enabled) => edit((r) => ({ ...r, sources: r.sources.map((source) => ids.includes(source.sheetId) ? { ...source, enabled } : source) }))}
            onPreview={(sheetId) => { setActiveSheet(sheetId);setSelectedCell(null);setSelectionError("");setFocusRow(undefined);setPhase("configure");setTab("source");setHomeView(false); }}
            onDelete={(fileId) => askDelete("files", [fileId])} />
          <div
            className="sidebar-resizer"
            role="separator"
            aria-orientation="vertical"
            aria-label="拖动调整文件列表宽度"
            title="左右拖动调整列表宽度，双击恢复默认"
            onPointerDown={handleResizerPointerDown}
            onDoubleClick={() => {
              setFileSidebarWidth(216);
              try {
                localStorage.setItem("gridflow_file_sidebar_width", "216");
              } catch {}
            }}
          />
        </aside>}
        <main className="main-pane">
          {phase === "results" && !isHome && <div className="result-toolbar"><div className="pane-tabs">
            {[
              { id: "preview", name: "合并预览", Icon: MousePointer2 },
              { id: "result", name: "汇总表", Icon: Layers },
              { id: "pivot", name: "分类统计", Icon: Table2 },
              { id: "chart", name: "图表", Icon: BarChart3 },
            ].map(({ id, name, Icon }) => (
              <button
                key={id}
                className={tab === id ? "active" : ""}
                onClick={() => {
                  setTab(id as typeof tab);

                  if (id === "preview") {
                    if (!preview && !busy && selectedSheets > 0) {
                      void previewRules();
                    }
                  } else if (id !== "source" && resultId) {
                    setQuery((old) => ({ ...old, datasetId: resultId }));
                  }
                }}
              >
                <Icon size={15} />
                {name}
                {id === "result" && ws?.datasets.length ? (
                  <span className="dot" />
                ) : null}
              </button>
            ))}
          </div><div className="result-tools">
            {tab !== "preview" && <select aria-label={tab === "result" ? "选择汇总表" : "分析数据来源"} value={dataset?.id ?? resultId ?? ""} onChange={(e) => {
              setResultId(e.target.value);setQuery(queryFor(e.target.value));setAnalysis(null);
            }}><option value="">选择汇总表</option>{ws?.datasets.map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}</select>}
            {tab === "preview" ? <Button icon={RefreshCw} disabled={!selectedSheets || !!busy} loading={busy === "规则预览"} onClick={() => void previewRules()}>刷新预览</Button>
              : tab === "pivot" || tab === "chart" ? <>
                <Button icon={RefreshCw} primary disabled={!dataset || !!busy} loading={busy === "计算分析"} onClick={() => void generateAnalysis()}>计算统计</Button>
                <Button icon={Download} disabled={!analysis || !!busy} onClick={() => void exportCurrent(tab === "chart" ? "image" : "pivot")}>保存{tab === "chart" ? "图表图片" : "统计表"}</Button>
              </> : null}
          </div></div>}
          {isHome ? (
            <div className="welcome">
              {!!ws?.files.length && (
                <div className="welcome-active-session">
                  <div className="session-info">
                    <CheckCircle2 size={16} className="session-icon" />
                    <span>当前工作区已导入 <b>{ws.files.length}</b> 个文件（{selectedSheets} 张表已选）</span>
                  </div>
                  <div className="session-actions">
                    <Button primary onClick={() => setHomeView(false)}>继续配置汇总</Button>
                    <Button danger icon={RotateCcw} onClick={confirmResetToInitial}>清空并重置</Button>
                  </div>
                </div>
              )}
              <div className="welcome-icon">
                <FileSpreadsheet size={28} />
              </div>
              <h1>
                把多份表格，合成一份 Excel。
              </h1>
              <p>
                添加文件，在 Excel 预览上选好表头和附加信息。
                <br />
                内容只在本机处理，原文件保持原样。
              </p>
              <div className="welcome-actions">
                <Button
                  icon={Plus}
                  primary
                  disabled={!!busy}
                  onClick={() => void importChoose()}
                >
                  添加文件
                </Button>
                <Button className="quiet" icon={FolderOpen} disabled={!!busy} onClick={() => void importChoose(true, true)}>从文件夹添加</Button>
                <Button icon={Sparkles} disabled={!!busy} loading={busy === "载入示例数据"} onClick={() => void loadSample()}>体验示例数据（2份连锁门店月报）</Button>
                <Button className="quiet" icon={BookOpen} onClick={() => setModal("help")}>快速上手教程</Button>
              </div>
              <div className="welcome-formats">
                <span>XLSX</span>
                <span>XLS</span>
                <span>CSV</span>
                <small>单文件 100MB · 每批 500MB</small>
              </div>
              <div className="welcome-steps">
                <span>
                  <b>1</b>添加文件
                </span>
                <ArrowRight size={15} />
                <span>
                  <b>2</b>配置表头与新列
                </span>
                <ArrowRight size={15} />
                <span>
                  <b>3</b>预览并保存
                </span>
              </div>
            </div>
          ) : tab === "source" ? (
            <>
              <div className="pane-toolbar configuration-toolbar">
                <Button className={`quiet panel-toggle ${panels.files ? "selected" : ""}`} icon={PanelLeft} aria-label={panels.files ? "收起文件列表" : "显示文件列表"} aria-expanded={panels.files} onClick={() => setPanels((old) => ({ ...old,files: !old.files }))}>文件</Button>
                <div className="source-context" title={`${ws?.files.find((f) => f.id===sheet?.fileId)?.name ?? ""} / ${sheet?.name ?? ""}`}><strong>{sheet?.name ?? "选择工作表"}</strong><small className="current-file-name">{ws?.files.find((f) => f.id===sheet?.fileId)?.name}</small></div>
                <div className="source-actions">
                  <Button data-tour="set-header-btn" icon={Table2} title="框选一行或多行列名后，将选区设为表头" disabled={!selection || !!busy} loading={busy === "检查表头"} onClick={() => void reviewHeader()}>设为表头</Button>
                  <Button data-tour="add-column-btn" icon={Plus} title="选择单个单元格，让每条记录带上这个位置的信息" disabled={!singleSelection || !selectedCell || !!busy} onClick={addSelectedCell}>添加为新列</Button>
                  <Button className="quiet" icon={Check} title="检查格式建议" disabled={!selectedSheets || !!busy} onClick={openReview}>格式建议</Button>
                  <Button className={`quiet panel-toggle ${panels.rules ? "selected" : ""}`} icon={PanelRight} aria-label={panels.rules ? "收起配置面板" : "显示配置面板"} aria-expanded={panels.rules} onClick={() => setPanels((old) => ({ ...old,rules: !old.rules }))}>配置</Button>
                  <Button className="quiet icon-button" icon={panels.files || panels.rules ? Maximize2 : Minimize2} aria-label={panels.files || panels.rules ? "专注表格" : "退出专注"} title={panels.files || panels.rules ? "专注表格：收起两侧面板" : "恢复之前的面板布局"} onClick={() => {
                    if (panels.files || panels.rules) { previousPanels.current = panels;setPanels({ files: false,rules: false }); }
                    else setPanels(previousPanels.current);
                  }} />
                </div>
              </div>
              {selectionError && <div className="notice error" role="alert">{selectionError}</div>}
              {sheet && activeSheet ? <ExcelPreview key={activeSheet} sheet={sheet} selection={selection} header={currentHeader} onSelect={selectArea} focusRow={focusRow} /> : <div className="empty-state">在左侧选择一张表，开始配置。</div>}
            </>
          ) : tab === "pivot" || tab === "chart" ? (
            <>
              {dataset ? (
                <div className="analysis-builder">
                  <div className="analysis-choices">
                    {([
                      ["rowFields", "1. 按什么分类？", "例如选「门店」，每家门店统计一行。"],
                      ["valueFields", "2. 要统计什么？", "例如选「金额」，计算各门店的销售总额。"],
                      ["columnFields", "再按什么横向对比？（可选）", "例如选「月份」，每个月单独显示一列。"],
                    ] as const).map(([key, title, help]) => (
                      <div className="analysis-choice" key={key}>
                        <label htmlFor={`analysis-${key}`}><strong>{title}</strong></label><small id={`analysis-${key}-help`}>{help}</small>
                        <select id={`analysis-${key}`} aria-label={title} aria-describedby={`analysis-${key}-help`} value="" onChange={(e) => { if (e.target.value) addField(key, e.target.value); }}>
                          <option value="">选择汇总表中的一列</option>
                          {dataset.columns.filter((c) => !pivot[key].includes(c.key)).map((c) => <option key={c.key} value={c.key}>{c.name}</option>)}
                        </select>
                        <span className="chosen-fields">{pivot[key].map((field) => <button type="button" key={field} aria-label={`移除${title} ${dataset.columns.find((c) => c.key === field)?.name}`} onClick={() => addField(key, field)}>
                          {dataset.columns.find((c) => c.key === field)?.name}<X size={12} />
                        </button>)}</span>
                      </div>
                    ))}
                  </div>
                  <div className="analysis-options">
                    <label>
                      统计方式
                      <select
                        value={pivot.aggregation}
                        onChange={(e) => {
                          setPivot((p) => ({
                            ...p,
                            aggregation: e.target.value,
                          }));
                          setAnalysis(null);
                        }}
                      >
                        {[
                          ["sum", "求和"],
                          ["avg", "平均值"],
                          ["count", "记录条数"],
                          ["min", "最小值"],
                          ["max", "最大值"],
                        ].map(([v, n]) => (
                          <option key={v} value={v}>
                            {n}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="check-label">
                      <input
                        type="checkbox"
                        checked={pivot.showRowTotals}
                        onChange={(e) => {
                          setPivot((p) => ({
                            ...p,
                            showRowTotals: e.target.checked,
                          }));
                          setAnalysis(null);
                        }}
                      />
                      显示总计
                    </label>
                    <Button
                      disabled={!dataset || !pivot.rowFields.length || !!busy}
                      onClick={() =>
                        void action("读取排序值", async () => {
                          const values = await call<string[]>(
                            "distinct_values",
                            {
                              query: { ...query, datasetId: dataset!.id },
                              key: pivot.rowFields[0],
                              offset: 0,
                            },
                          );
                          setCustomValues([
                            ...pivot.customOrder.filter((v) =>
                              values.includes(v),
                            ),
                            ...values.filter(
                              (v) => !pivot.customOrder.includes(v),
                            ),
                          ]);
                          setModal("customSort");
                        })
                      }
                    >
                      自定义顺序
                    </Button>
                    {tab === "chart" && (
                      <label>
                        图表
                        <select
                          aria-label="图表类型"
                          value={chartType}
                          onChange={(e) => setChartType(e.target.value)}
                        >
                          {[
                            ["bar", "柱状图"],
                            ["line", "折线图"],
                            ["pie", "饼图"],
                            ["scatter", "散点图"],
                          ].map(([v, n]) => (
                            <option key={v} value={v}>
                              {n}
                            </option>
                          ))}
                        </select>
                      </label>
                    )}
                    {analysis && (
                      <label>
                        排序
                        <select
                          value={pivot.sort?.key ?? ""}
                          onChange={(e) =>
                            setPivot((p) => ({
                              ...p,
                              sort: e.target.value
                                ? { key: e.target.value, descending: false }
                                : null,
                            }))
                          }
                        >
                          <option value="">默认顺序</option>
                          {analysis.columns.map((c) => (
                            <option value={c.key} key={c.key}>
                              {c.name}
                            </option>
                          ))}
                        </select>
                        <button
                          onClick={() =>
                            setPivot((p) => ({
                              ...p,
                              sort: p.sort
                                ? { ...p.sort, descending: !p.sort.descending }
                                : null,
                            }))
                          }
                        >
                          {pivot.sort?.descending ? "降序" : "升序"}
                        </button>
                      </label>
                    )}
                    <Button
                      onClick={() => {
                        setPivot((p) => ({
                          ...p,
                          rowFields: [],
                          columnFields: [],
                          valueFields: [],
                          sort: null,
                          customOrder: [],
                        }));
                        setAnalysis(null);
                      }}
                    >
                      清空
                    </Button>
                  </div>
                </div>
              ) : null}
              {analysis ? (
                <>
                  {analysis.truncated && (
                    <div className="notice">
                      显示前 2,000 个分组，统计基于全部{" "}
                      {analysis.totalRecords.toLocaleString()}{" "}
                      条记录，导出包含完整分组。
                    </div>
                  )}
                  {tab === "pivot" ? (
                    <StaticGrid
                      columns={analysis.columns}
                      rows={analysis.rows}
                    />
                  ) : (
                    <Suspense
                      fallback={
                        <div className="empty-state">正在加载图表…</div>
                      }
                    >
                      <Chart
                        analysis={analysis}
                        type={chartType}
                        onReady={(png) => {
                          chartPng.current = png;
                        }}
                        onFilter={(value) => {
                          if (pivot.rowFields.length === 1) {
                            setQuery((q) => ({
                              ...q,
                              filters: [
                                ...q.filters.filter(
                                  (f) => f.key !== pivot.rowFields[0],
                                ),
                                {
                                  key: pivot.rowFields[0],
                                  operator: "in",
                                  values: [value],
                                },
                              ],
                            }));
                            setTab("result");setPhase("results");
                          }
                        }}
                      />
                    </Suspense>
                  )}
                  <div className="table-status">
                    <span>
                      {analysis.totalRecords.toLocaleString()} 条完整记录 ·{" "}
                      {analysis.groupCount.toLocaleString()} 个分组
                    </span>
                    <span>当前筛选条件同步用于统计与导出</span>
                  </div>
                </>
              ) : (
                <div className="empty-state">
                  <BarChart3 size={40} />
                  <h2>
                    {dataset
                      ? "选一个分类列和一个统计列，再点击「计算统计」"
                      : "先合并表格，或选择一份已有的汇总表"}
                  </h2>
                  <p>例如：按门店分类，计算金额总和，就能看到每家门店的销售额。</p>
                </div>
              )}
            </>
          ) : (
            <>
              {dataset ? (
                <>
                  {staleResult && <div className="notice" role="status">配置已修改，当前结果仍是修改前的版本。点击「按当前配置刷新结果」查看新内容。</div>}
                  <div className="result-summary">
                    <span>
                      <b>{dataset.summary.outputRows.toLocaleString()}</b>{" "}
                      条数据
                    </span>
                    <span>{dataset.columns.length} 列</span>
                    {dataset.summary.duplicatesRemoved > 0 && <span>
                      删除了 {dataset.summary.duplicatesRemoved.toLocaleString()} 条重复记录
                    </span>}
                    <span>
                      {(dataset.summary.elapsedMs / 1000).toFixed(2)} 秒
                    </span>
                    {dataset.summary.issueCount > 0 ? (
                      <button
                        className="issue-link"
                        onClick={() => void showIssues(dataset)}
                      >
                        <AlertTriangle size={14} />
                        {dataset.summary.issueCount} 处内容需要检查 · 查看
                      </button>
                    ) : (
                      <span className="success-text">
                        <Check size={14} />
                        合并完成
                      </span>
                    )}
                  </div>
                  {dataset.sample && (
                    <div className="notice">
                      样本预览：每个勾选工作表的前 200
                      行。去重与统计仅覆盖样本，完整统计以正式结果为准。
                    </div>
                  )}
                  <div className="filter-bar">
                    <Search size={15} />
                    <input
                      aria-label="搜索结果内容"
                      placeholder="搜索结果内容"
                      value={query.search}
                      onChange={(e) =>
                        setQuery((q) => ({
                          ...q,
                          search: e.target.value,
                          offset: 0,
                        }))
                      }
                    />
                    <select
                      aria-label="选择要筛选的列"
                      value=""
                      onChange={(e) => {
                        if (e.target.value) void loadValues(e.target.value);
                      }}
                    >
                      <option value="">选择一列，勾选要保留的内容</option>
                      {dataset.columns.map((c) => (
                        <option key={c.key} value={c.key}>
                          {c.name}
                        </option>
                      ))}
                    </select>
                    {query.filters.map((f) => (
                      <button
                        key={f.key}
                        className="filter-chip"
                        onClick={() =>
                          setQuery((q) => ({
                            ...q,
                            filters: q.filters.filter((x) => x !== f),
                          }))
                        }
                      >
                        {dataset.columns.find((c) => c.key === f.key)?.name}:{" "}
                        {f.values
                          .map((v) => v || "(空白)")
                          .slice(0, 2)
                          .join("、")}
                        {f.values.length > 2 ? "…" : ""}
                        <X size={12} />
                      </button>
                    ))}
                    {(query.filters.length > 0 || query.search) && (
                      <button onClick={() => setQuery(queryFor(dataset.id))}>
                        清除筛选
                      </button>
                    )}
                  </div>
                  <Grid
                    columns={dataset.columns.filter(
                      (c) =>
                        !dataset.rules.columns.some(
                          (r) => r.key === c.key && r.hidden,
                        ),
                    )}
                    load={(offset, limit, sorts) =>
                      call<DataPage>("query_dataset", {
                        query: {
                          ...query,
                          datasetId: dataset.id,
                          offset,
                          limit,
                          sorts: sorts.length ? sorts : query.sorts,
                        },
                      }).catch((e) => {
                        error(e);
                        throw e;
                      })
                    }
                    version={`result:${dataset.id}:${JSON.stringify(query)}`}
                  />
                  <div className="table-status">
                    <span>点击列名可排序，拖动可调整显示顺序；输出列在配置页设置</span>
                    <span>内容仅存本机</span>
                  </div>
                </>
              ) : (
                <div className="empty-state">
                  <Layers size={40} />
                  <h2>
                    {tab === "preview"
                      ? "快速检查前 200 条 / 表"
                      : running ? "正在生成结果预览…" : stoppedTask ? `预览${taskNames[stoppedTask.state]}` : "配置完成后，在这里查看结果"}
                  </h2>
                  <p>
                    {tab === "preview"
                      ? "先看每张表的前 200 条数据；满意后再合并全部内容。"
                      : running ? "完成后自动显示全部数据，配置页仍可返回。" : stoppedTask ? stoppedTask.error ?? "配置已保留，可以返回修改或重新预览。" : "返回配置页，确认内容后点击「预览结果」。"}
                  </p>
                  {tab === "preview" && !running && (
                    <Button
                      primary
                      icon={Play}
                      disabled={!selectedSheets || !!busy}
                      loading={busy === "规则预览"}
                      onClick={() => void previewRules()}
                    >
                      生成合并预览
                    </Button>
                  )}
                  {tab !== "preview" && !running && (
                    <Button
                      primary
                      icon={Play}
                      disabled={!selectedSheets || !!busy}
                      onClick={() => void generate()}
                    >
                      重新生成结果
                    </Button>
                  )}
                </div>
              )}
            </>
          )}
        </main>
        {phase === "configure" && !isHome && !!ws?.files.length && panels.rules && (
          <aside className="rules-panel">
            <div className="section-title">
              <span>汇总配置</span>
              <span className="muted">自动保存</span>
            </div>
            <div className="panel-tabs">
              {[
                ["sheets", "表头"],
                ["columns", "输出列"],
                ["clean", "整理"],
                ["fixed", "新增列"],
              ].map(([id, name]) => (
                <button
                  key={id}
                  className={panelTab === id ? "active" : ""}
                  onClick={() => setPanelTab(id)}
                >
                  {name}
                  {id === "fixed" && rules.fixedCells.length ? (
                    <span>{rules.fixedCells.length}</span>
                  ) : null}
                </button>
              ))}
            </div>
            <div className="panel-scroll">
              {panelTab === "sheets" ? (
                <>
                  <div className="panel-heading">
                    <h3>表头与读取范围</h3>
                    <p>框选列名所在的一行或多行，再点「设为表头」。</p>
                  </div>
                  {sheet && source ? (
                    <>
                      <div className="header-settings">
                        <div className="setting-row">
                          <span>当前表头</span>
                          <button
                            className="text-button"
                            onClick={() => {
                              setTab("source");
                              setSelections((old) => ({ ...old,[sheet.id]: currentHeader }));setPhase("configure");
                            }}
                          >
                            {source.headerRange ? rangeLabel(source.headerRange) : `第 ${source.headerRow} 行`} · 修改
                          </button>
                        </div>
                        <div className="setting-row"><span>数据从</span><strong>第 {source.headerRow + 1} 行开始</strong></div>
                        {!source.headerRange && sheet.suggestedHeaderRow !== source.headerRow && (
                          <button
                            className="suggestion-link"
                            onClick={openReview}
                          >
                            第 {sheet.suggestedHeaderRow} 行可能才是列名，检查一下
                          </button>
                        )}
                      </div>
                      <Button
                        icon={Copy}
                        className="full"
                        onClick={() =>
                          edit((r) => ({
                            ...r,
                            sources: r.sources.map((s) => {
                              const target = ws?.files
                                .flatMap((f) => f.sheets)
                                .find((x) => x.id === s.sheetId);
                              return target?.name === sheet.name && s.enabled
                                ? { ...s, headerRow: source.headerRow,headerRange:source.headerRange }
                                : s;
                            }),
                          }))
                        }
                      >
                        应用到勾选的同名表
                      </Button>
                    </>
                  ) : (
                    <p className="muted">从左侧选择一个工作表。</p>
                  )}
                  <label className="stacked-label">
                    按表名批量选择
                    <select
                      value=""
                      onChange={(e) => {
                        if (!e.target.value) return;
                        const ids =
                          ws?.files.flatMap((f) =>
                            f.sheets
                              .filter((s) => s.name === e.target.value)
                              .map((s) => s.id),
                          ) ?? [];
                        edit((r) => ({
                          ...r,
                          sources: r.sources.map((s) => ({
                            ...s,
                            enabled: ids.includes(s.sheetId),
                          })),
                        }));
                      }}
                    >
                      <option value="">选择要合并的工作表名称</option>
                      {[
                        ...new Set(
                          ws?.files.flatMap((f) =>
                            f.sheets.map((s) => s.name),
                          ) ?? [],
                        ),
                      ].map((name) => (
                        <option key={name}>{name}</option>
                      ))}
                    </select>
                  </label>
                  <label className="stacked-label">
                    文件列数不同时
                    <select
                      value={rules.mergeStrategy}
                      onChange={(e) =>
                        edit((r) => ({ ...r, mergeStrategy: e.target.value }))
                      }
                    >
                      <option value="outer">全部保留，缺少的内容留空</option>
                      <option value="inner">只保留每个文件都有的列</option>
                    </select>
                  </label>
                  <label className="check-label">
                    <input
                      type="checkbox"
                      checked={rules.includeSource}
                      onChange={(e) =>
                        edit((r) => ({ ...r, includeSource: e.target.checked }))
                      }
                    />
                    附加来源文件名
                  </label>
                  {source && rules.includeSource && (
                    <details className="advanced">
                      <summary>自定义这份文件的来源名称</summary>
                      <label className="stacked-label">汇总表中显示为
                        <input aria-label="来源名称" value={source.sourceName}
                          onChange={(e) => edit((r) => ({ ...r, sources: r.sources.map((s) =>
                            s.sheetId === activeSheet ? { ...s, sourceName: e.target.value } : s,
                          ) }))} />
                      </label>
                    </details>
                  )}
                  <details className="panel-help">
                    <summary><Info size={13} />表头与合并说明</summary>
                    <p>支持框选多层表头。合并单元格中的上层名称，会与下层列名组合，例如「销售 / 金额」。</p>
                    <p>{rules.mergeStrategy === "outer" ? "只有部分文件有「备注」时，仍保留这列，其他文件留空。" : "只有部分文件有「备注」时，汇总表不包含这列。"}</p>
                    <p>来源列默认填写文件名，用来查找每条记录来自哪份原表。</p>
                  </details>
                </>
              ) : panelTab === "columns" ? (
                <>
                  <div className="panel-heading">
                    <h3>汇总表要保留哪些列？</h3>
                    <p>勾选要保留的列，改名或用箭头调整顺序。</p>
                  </div>
                  {allColumns.map((col, i) => (
                    <div className="column-rule" key={col.key}>
                      <div className="column-rule-head">
                        <input
                          aria-label={`导出 ${col.name}`}
                          type="checkbox"
                          checked={!col.hidden}
                          onChange={(e) =>
                            updateColumn(col.key, { hidden: !e.target.checked })
                          }
                        />
                        <input
                          className="column-name"
                          aria-label={`列名 ${col.key}`}
                          value={col.name}
                          onChange={(e) =>
                            updateColumn(col.key, { name: e.target.value })
                          }
                        />
                        <button
                          aria-label={`${col.name} 上移`}
                          disabled={i === 0}
                          onClick={() => moveColumn(col.key, -1)}
                        >
                          <ArrowUp size={13} />
                        </button>
                        <button
                          aria-label={`${col.name} 下移`}
                          disabled={i === allColumns.length - 1}
                          onClick={() => moveColumn(col.key, 1)}
                        >
                          <ArrowDown size={13} />
                        </button>
                      </div>
                      <select
                        aria-label={`${col.name} 类型`}
                        value={col.dataType}
                        onChange={(e) =>
                          updateColumn(col.key, {
                            dataType: e.target.value as ColumnRule["dataType"],
                          })
                        }
                      >
                        {Object.entries(typeNames).map(([v, n]) => (
                          <option key={v} value={v}>
                            {n}
                          </option>
                        ))}
                      </select>
                      {col.dataType === "date" && (
                        <select
                          aria-label={`${col.name} 日期格式`}
                          value={rules.dateFormats[col.key] ?? "auto"}
                          onChange={(e) =>
                            edit((r) => ({
                              ...r,
                              dateFormats: {
                                ...r.dateFormats,
                                [col.key]: e.target.value,
                              },
                            }))
                          }
                        >
                          <option value="auto">
                            自动识别；不能确定时保留原值
                          </option>
                          <option value="ymd">年月日</option>
                          <option value="dmy">日月年</option>
                          <option value="mdy">月日年</option>
                        </select>
                      )}
                    </div>
                  ))}
                  <details className="advanced">
                    <summary>不同列名，其实是同一项内容？</summary>
                    <p>例如：把「销售额」归到「金额」，合并后它们放在同一列。这里只指定对应关系；修改上方列名可改显示名称。</p>
                    {inputColumns
                      .filter(
                        (c) =>
                          !c.key.startsWith("fixed:") && c.key !== "__source",
                      )
                      .map((col) => (
                        <label className="mapping-row" key={col.key}>
                          <span>{col.name}</span>
                          <select
                            aria-label={`映射 ${col.name}`}
                            value={
                              rules.mappings.find(
                                (m) => m.sourceKey === col.key,
                              )?.targetKey ?? col.key
                            }
                            onChange={(e) =>
                              edit((r) => ({
                                ...r,
                                mappings: [
                                  ...r.mappings.filter(
                                    (m) => m.sourceKey !== col.key,
                                  ),
                                  ...(e.target.value === col.key
                                    ? []
                                    : [
                                        {
                                          sourceKey: col.key,
                                          targetKey: e.target.value,
                                        },
                                      ]),
                                ],
                              }))
                            }
                          >
                            {inputColumns.map((target) => (
                              <option key={target.key} value={target.key}>
                                {target.name}
                              </option>
                            ))}
                          </select>
                        </label>
                      ))}
                  </details>
                </>
              ) : panelTab === "clean" ? (
                <>
                  <div className="panel-heading">
                    <h3>需要删除或整理哪些数据？</h3>
                    <p>这些选项只影响汇总表。不能转成数字或日期的内容会保留原值，完成后可以逐项查看。</p>
                  </div>
                  {[
                    ["removeEmptyRows", "删除空白行", "默认只删除整行都没有内容的记录。"],
                    ["removeDuplicates", "重复记录只留一条", "可以指定按订单号等列判断是否重复。"],
                    ["cleanNumeric", "让金额、数量可以计算", "将识别出的数字列中的 ¥1,200 等内容转成数字。"],
                    ["parseDates", "把日期统一成日期格式", "将识别出的日期列转成可按时间排序的日期。"],
                  ].map(([key, name, description]) => (
                    <label className="option-row" key={key}>
                      <input
                        type="checkbox"
                        checked={Boolean(rules[key as keyof RuleSet])}
                        onChange={(e) =>
                          edit((r) => ({ ...r, [key]: e.target.checked }))
                        }
                      />
                      <span>{name}<small>{description}</small></span>
                    </label>
                  ))}
                  {rules.removeDuplicates && (
                    <>
                      <label className="stacked-label">
                        同一条记录出现多次，保留
                        <select
                          value={rules.keepStrategy}
                          onChange={(e) =>
                            edit((r) => ({
                              ...r,
                              keepStrategy: e.target.value,
                            }))
                          }
                        >
                          <option value="first">最先读到的那条</option>
                          <option value="last">最后读到的那条</option>
                        </select>
                      </label>
                      <div className="field-checks">
                        <span className="muted">
                          哪些列相同就算重复？不选则要求所有数据列都相同。
                        </span>
                        {columns
                          .filter((c) => c.key !== "__source")
                          .map((c) => (
                            <label key={c.key}>
                              <input
                                type="checkbox"
                                checked={rules.duplicateColumns.includes(c.key)}
                                onChange={(e) =>
                                  edit((r) => ({
                                    ...r,
                                    duplicateColumns: e.target.checked
                                      ? [...r.duplicateColumns, c.key]
                                      : r.duplicateColumns.filter(
                                          (k) => k !== c.key,
                                        ),
                                  }))
                                }
                              />
                              {c.name}
                            </label>
                          ))}
                      </div>
                    </>
                  )}
                  {rules.removeEmptyRows && (
                    <div className="field-checks">
                      <span className="muted">
                        哪些列都为空时删除这行？不选则要求整行都为空。
                      </span>
                      {inputColumns.map((c) => (
                        <label key={c.key}>
                          <input
                            type="checkbox"
                            checked={rules.emptyColumns.includes(c.key)}
                            onChange={(e) =>
                              edit((r) => ({
                                ...r,
                                emptyColumns: e.target.checked
                                  ? [...r.emptyColumns, c.key]
                                  : r.emptyColumns.filter((k) => k !== c.key),
                              }))
                            }
                          />
                          {c.name}
                        </label>
                      ))}
                    </div>
                  )}
                  <div className="help-note">
                    <AlertTriangle size={14} />
                    <span>
                      像「00123」这样的编号建议保存为文字，避免丢掉开头的 0。金额显示的小数位不影响原始精度。
                    </span>
                  </div>
                </>
              ) : (
                <>
                  <div className="panel-heading">
                    <h3>把表格外的信息带进来</h3>
                    <p>选择原表中的单个单元格，再点「添加为新列」。</p>
                  </div>
                  <Button
                    icon={MousePointer2}
                    className="full"
                    disabled={!sheet}
                    onClick={() => {
                      setTab("source");
                      setPhase("configure");setSelectionError("请在左边原表中选一个单元格，再点击上方的加为新列按钮。");
                    }}
                  >
                    去表格中点选单元格
                  </Button>
                  {rules.fixedCells.map((rule) => (
                    <div className="setting-card" key={rule.id}>
                      <div className="setting-row">
                        <strong>{rule.name}</strong>
                        <button
                          aria-label={`删除固定值 ${rule.name}`}
                          onClick={() =>
                            edit((r) => ({
                              ...r,
                              fixedCells: r.fixedCells.filter(
                                (x) => x.id !== rule.id,
                              ),
                            }))
                          }
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                      <span className="coordinate-badge">
                        {rule.sheetName} ! {rule.address}
                      </span>
                      <small>每份文件各自读这个位置，再填到该文件的每一行</small>
                    </div>
                  ))}
                  {!rules.fixedCells.length && (
                    <p className="muted">也适用于写在表格上方的报表日期、部门或负责人。</p>
                  )}
                </>
              )}
            </div>
            <div className="panel-footer">
              <Button
                icon={MousePointer2}
                disabled={!selectedSheets || !!busy}
                loading={busy === "规则预览"}
                onClick={() => void previewRules()}
              >
                检查前 200 条 / 表
              </Button>
            </div>
          </aside>
        )}
      </div>
      <footer className="task-tray">
        <div className="task-tray-status">
          {running ? (
            <>
              <span className="pulse-dot" />
              <b>{taskNames[running.state]}</b>
              <span>{running.phase}</span>
              <div className="progress-track">
                <i style={{ width: `${running.progress}%` }} />
              </div>
              <span>{running.progress}%</span>
              <Button
                disabled={running.state === "cancelling"}
                onClick={() =>
                  void call("cancel_task", { taskId: running.id }).catch(error)
                }
              >
                取消
              </Button>
            </>
          ) : (
            <>
              <span className="status-dot" />
              <span>{busy || "工作台就绪"}</span>
              <small>{phase === "configure" && ws?.files.length ? `已选 ${selectedFiles.length} 个文件${selectedSheets === selectedFiles.length ? "" : ` · ${selectedSheets} 张表`} · 约 ${estimatedRows.toLocaleString()} 条` : "文件在本机处理 · 配置自动保存"}</small>
            </>
          )}
        </div>
        <button className="text-button" onClick={() => setModal("tasks")}>
          处理记录 <ChevronUp size={14} />
        </button>
      </footer>
      {modal === "imports" && (
        <Modal title="导入结果" onClose={() => setModal(null)}>
          <div className="modal-body">
            <p>
              成功 {importReport.filter((i) => i.file).length} 个，失败{" "}
              {importReport.filter((i) => i.error).length} 个。
            </p>
            {importReport.map((item, i) => (
              <div className="import-row" key={`${item.path}:${i}`}>
                {item.file ? (
                  <Check size={16} className="success-text" />
                ) : (
                  <AlertTriangle size={16} className="error-text" />
                )}
                <div>
                  <strong>{item.path.split(/[\\/]/).at(-1)}</strong>
                  <small>
                    {item.error ?? `${item.file!.sheets.length} 个工作表已导入`}
                  </small>
                </div>
                {item.error && (
                  <Button
                    disabled={!!busy}
                    onClick={() => void importPaths([item.path], true)}
                  >
                    重试
                  </Button>
                )}
              </div>
            ))}
          </div>
          <div className="modal-footer">
            <Button primary onClick={() => setModal(null)}>
              查看已添加的文件
            </Button>
          </div>
        </Modal>
      )}
      {modal === "suggest" && (
        <Modal title="合并前检查" onClose={() => setModal(null)}>
          <div className="modal-body review-body">
            <div className="review-intro">
              <Check size={22} />
              <div>
                <h3>{hintsLoading ? "正在检查所选表格…" : reviewError ? "检查未完成" : "可以按当前设置合并"}</h3>
                <p>{selectedFiles.length} 个文件 · {selectedSheets} 张表 · 约 {estimatedRows.toLocaleString()} 条数据</p>
              </div>
            </div>
            <p>合并时，同名列会放在一起，记录按文件接到下面。下面的调整可以按需选择，原文件不会改动。</p>
            {headerChanges.length ? (
              <section className="review-section">
                <h3>有 {headerChanges.length} 张表的列名位置值得检查</h3>
                <p>列名行决定从哪里开始读数据。程序的判断只是建议，请对照原始表格确认。</p>
                {headerGroups.map((group) => (
                  <details className="review-detail" key={`${group.name}:${group.current}:${group.suggested}`}>
                    <summary>「{group.name}」{group.items.length} 张表：当前第 {group.current} 行，建议第 {group.suggested} 行</summary>
                    <ul>{group.items.map((item) => <li key={item.sheet.id}>{item.file.name}</li>)}</ul>
                  </details>
                ))}
                <label className="option-row">
                  <input type="checkbox" checked={suggestHeaders} onChange={(e) => {
                    setSuggestHeaders(e.target.checked);
                    setSelectedHintKeys([]);
                  }} />
                  <span>使用建议的列名行<small>影响以上 {headerChanges.length} 张表；从建议行的下一行开始合并。</small></span>
                </label>
              </section>
            ) : (
              <div className="review-status"><Check size={15} />列名位置与自动识别一致，无需调整。</div>
            )}
            {reviewError ? (
              <div className="review-section" role="alert">
                <p className="error-text">{reviewError}</p>
                <Button onClick={() => setReviewAttempt((n) => n + 1)}>重新检查</Button>
              </div>
            ) : hintsLoading ? <p role="status">正在读取数字和日期的样本…</p> : usefulHints.length ? (
              <section className="review-section">
                <h3>可选的格式调整</h3>
                <p>只列出可能需要改变的列。无需逐列确认，也可以按当前设置合并。</p>
                {usefulHints.map((hint) => {
                  const example = hint.changes[0];
                  const kindName = (cell: Cell) => cell.kind === "number" ? "数字" : cell.kind === "date" ? "日期" : "文字";
                  return (
                    <div className="format-review" key={hint.key}>
                      <label className="option-row">
                        <input type="checkbox" aria-label={`调整 ${hint.name}`}
                          checked={selectedHintKeys.includes(hint.key)} disabled={!hint.changes.length}
                          onChange={(e) => setSelectedHintKeys((keys) => e.target.checked ? [...keys, hint.key] : keys.filter((k) => k !== hint.key))} />
                        <span><strong>{hint.name}</strong><small>{formatPurpose(hint.suggestedType)}</small></span>
                      </label>
                      {example && <div className="format-example">
                        <span>{kindName(example.before)} <code>{cellText(example.before)}</code></span>
                        <ArrowRight size={14} />
                        <span>{kindName(example.after)} <code>{cellText(example.after)}</code></span>
                      </div>}
                      {hint.errors.length > 0 && <p className="error-text">样本中有 {hint.errors.length} 个值无法转换，将保留原样。例如「{cellText(hint.errors[0].before)}」：{hint.errors[0].error}</p>}
                    </div>
                  );
                })}
                <p className="review-scope">格式调整影响所选表格的同名列。这里只检查了每列最多 3 个非空样本；合并全部数据后，会列出未能转换的内容。</p>
              </section>
            ) : <div className="review-status"><Check size={15} />样本中没有需要调整的格式。</div>}
          </div>
          <div className="modal-footer">
            <Button onClick={() => setModal(null)}>返回查看文件</Button>
            {hasReviewSelection && <Button disabled={hintsLoading || !!reviewError} onClick={() => applySuggestions()}>仅使用调整</Button>}
            <Button primary icon={Play} disabled={hintsLoading || !!reviewError || !!busy} onClick={() => applySuggestions(true)}>
              {hasReviewSelection ? "调整后合并表格" : "按当前设置合并"}
            </Button>
          </div>
        </Modal>
      )}
      {modal === "fixed" && fixedCandidate && (
        <Modal title="把单元格内容加到每条记录" onClose={() => setModal(null)}>
          <div className="modal-body">
            <div className="cell-preview">
              <span>
                {fixedCandidate.sheet.name} !{" "}
                {coordinate(fixedCandidate.row, fixedCandidate.column)}
              </span>
              <strong>{cellDisplay(fixedCandidate.value)}</strong>
            </div>
            <label className="stacked-label">
              这列叫什么？（已有默认名称）
              <input
                value={saveName}
                onChange={(e) => setSaveName(e.target.value)}
              />
            </label>
            <p>每份文件都从同名表的这个位置取内容，填写到自己的每条记录上。例如 B2 是门店名称，就可以给每条销售记录加上门店。</p>
          </div>
          <div className="modal-footer">
            <Button onClick={() => setModal(null)}>取消</Button>
            <Button
              primary
              onClick={() => {
                edit((r) => ({
                  ...r,
                  fixedCells: [
                    ...r.fixedCells,
                    {
                      id: crypto.randomUUID(),
                      name:
                        saveName.trim() ||
                        `${fixedCandidate.sheet.name} ${coordinate(fixedCandidate.row, fixedCandidate.column)}`,
                      sheetName: fixedCandidate.sheet.name,
                      address: coordinate(
                        fixedCandidate.row,
                        fixedCandidate.column,
                      ),
                    },
                  ],
                }));
                setSaveName("");
                setModal(null);
                setPanelTab("fixed");
              }}
            >
              加入汇总表
            </Button>
          </div>
        </Modal>
      )}
      {modal === "header" && headerCandidate && (
        <Modal title="把这些行作为表头？" onClose={() => { setModal(null);setSelectionError(""); }}>
          <div className="modal-body">
            <div className="cell-preview"><span>已框选 {rangeLabel(headerCandidate.range)}</span><strong>{headerCandidate.range.endRow-headerCandidate.range.startRow+1} 行表头 · {headerCandidate.columns.length} 列</strong></div>
            <p>下面是合并后的列名。多层标题用「/」连接；数据从第 {headerCandidate.range.endRow+1} 行开始读取。</p>
            <div className="header-name-preview">{headerCandidate.columns.map((col) => <span key={col.key}>{col.name}</span>)}</div>
            {selectedItems.filter((item) => item.sheet.id!==sheet?.id && item.sheet.name===sheet?.name).length > 0 && <label className="option-row">
              <input type="checkbox" checked={headerScope} onChange={(e) => { setHeaderScope(e.target.checked);setSelectionError(""); }} />
              <span>同时用于其余 {selectedItems.filter((item) => item.sheet.id!==sheet?.id && item.sheet.name===sheet?.name).length} 张「{sheet?.name}」表<small>各文件都使用相同的行列位置，无需逐个重复选择。</small></span>
            </label>}
            {selectionError && <div className="notice error" role="alert">{selectionError}</div>}
          </div>
          <div className="modal-footer"><Button onClick={() => { setModal(null);setSelectionError(""); }}>重新框选</Button><Button primary icon={Check} onClick={applyHeader}>使用这片表头</Button></div>
        </Modal>
      )}
      {modal === "save" && (
        <Modal title="保存设置，下次直接使用" onClose={() => setModal(null)}>
          <div className="modal-body">
            <label className="stacked-label">
              给这套设置起个名字（可以不填）
              <input
                autoFocus
                placeholder="例如：月度销售汇总"
                value={saveName}
                onChange={(e) => setSaveName(e.target.value)}
              />
            </label>
            <p>
              保存选了哪些表、从哪行开始读、保留哪些列，以及整理数据的选项。下次添加同类文件后，在「配置文件」里选它即可。
            </p>
            {saveName.trim() &&
              !!ws?.schemes.some((s) => s.name === saveName.trim()) && (
                <p className="muted">
                  已有同名设置「{saveName.trim()}」，保存会覆盖它。
                </p>
              )}
            {noticeBox}
          </div>
          <div className="modal-footer">
            <Button onClick={() => setModal(null)}>取消</Button>
            <Button
              icon={Save}
              primary
              loading={busy === "保存方案"}
              onClick={() => void saveScheme()}
            >
              保存设置
            </Button>
          </div>
        </Modal>
      )}
      {modal === "reuse" && (
        <Modal title="配置文件 · 方案管理" onClose={() => setModal(null)}>
          <div className="modal-body">
            <h3>使用以前保存的设置</h3>
            <p>新添加的文件会按相同表名匹配，复用列名行、保留列和整理数据的选项。文件内容仍来自本次勾选的文件。</p>
            {ws?.schemes.length ? (
              <>
                <label className="stacked-label">选择一套设置
                  <select aria-label="选择已保存方案" value={schemeId} onChange={(e) => { setSchemeId(e.target.value); setRenameName(""); }}>
                    <option value="">选择保存过的设置</option>
                    {ws.schemes.slice().reverse().map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
                  </select>
                </label>
                <div className="toolbar-group">
                  <input
                    aria-label="所选设置的新名称"
                    placeholder="填新名字可重命名"
                    value={renameName}
                    onChange={(e) => setRenameName(e.target.value)}
                  />
                  <Button
                    icon={Pencil}
                    disabled={!schemeId || !renameName.trim() || !!busy}
                    loading={busy === "重命名方案"}
                    onClick={() => void renameScheme()}
                  >
                    重命名
                  </Button>
                  <Button
                    danger
                    icon={Trash2}
                    disabled={!schemeId || !!busy}
                    onClick={() => askDelete("schemes", [schemeId])}
                  >
                    删除所选
                  </Button>
                </div>
              </>
            ) : <p className="empty-note">还没有保存过设置，可以先保存这次的。</p>}
            <div className="review-section">
              <h3>保存这次的设置</h3>
              <p>下次只需添加文件、使用设置、合并表格。</p>
              <Button icon={Save} onClick={() => { setSaveName(""); setModal("save"); }}>保存这次的设置</Button>
            </div>
            {noticeBox}
          </div>
          <div className="modal-footer">
            <Button onClick={() => setModal(null)}>返回</Button>
            <Button primary disabled={!schemeId || !!busy || !selectedSheets} loading={busy === "应用方案"} onClick={() => void applyScheme()}>使用所选设置</Button>
          </div>
        </Modal>
      )}
      {modal === "filter" && (
        <Modal
          title={`筛选 · ${dataset?.columns.find((c) => c.key === filterKey)?.name ?? filterKey}`}
          onClose={() => setModal(null)}
        >
          <div className="modal-body">
            <input
              aria-label="查找筛选值"
              placeholder="查找可选值"
              value={filterSearch}
              onChange={(e) => setFilterSearch(e.target.value)}
            />
            <div className="filter-quick">
              <button onClick={() => setChosenValues(filterValues)}>
                全选已加载值
              </button>
              <button onClick={() => setChosenValues([])}>取消全选</button>
              <span>已选 {chosenValues.length}</span>
            </div>
            <div className="filter-values">
              {filterValues
                .filter((v) =>
                  v.toLowerCase().includes(filterSearch.toLowerCase()),
                )
                .map((value) => (
                  <label key={value}>
                    <input
                      type="checkbox"
                      checked={chosenValues.includes(value)}
                      onChange={(e) =>
                        setChosenValues((old) =>
                          e.target.checked
                            ? [...old, value]
                            : old.filter((v) => v !== value),
                        )
                      }
                    />
                    {value || "(空白)"}
                  </label>
                ))}
            </div>
            {filterValues.length >= filterOffset + 200 && (
              <Button
                disabled={!!busy}
                onClick={() => void loadValues(filterKey, filterOffset + 200)}
              >
                加载更多可选值
              </Button>
            )}
            {noticeBox}
          </div>
          <div className="modal-footer">
            <Button onClick={() => setModal(null)}>取消</Button>
            <Button
              primary
              onClick={() => {
                setQuery((q) => ({
                  ...q,
                  offset: 0,
                  filters: [
                    ...q.filters.filter((f) => f.key !== filterKey),
                    ...(chosenValues.length
                      ? [
                          {
                            key: filterKey,
                            operator: "in",
                            values: chosenValues,
                          },
                        ]
                      : []),
                  ],
                }));
                setAnalysis(null);
                setModal(null);
              }}
            >
              应用筛选
            </Button>
          </div>
        </Modal>
      )}
      {modal === "history" && (
        <Modal title="历史结果与批量操作" wide onClose={() => setModal(null)}>
          <div className="modal-body">
            <div className="toolbar-group">
              <Button
                onClick={() =>
                  setSelectedResults(ws?.datasets.map((d) => d.id) ?? [])
                }
              >
                全选
              </Button>
              <Button onClick={() => setSelectedResults([])}>取消选择</Button>
              <select
                value={exportFormat}
                aria-label="批量导出格式"
                onChange={(e) => setExportFormat(e.target.value)}
              >
                <option value="xlsx">Excel</option>
                <option value="csv">CSV</option>
              </select>
              <Button
                icon={Download}
                disabled={!selectedResults.length || !!busy}
                onClick={() => void batchExport()}
              >
                保存到文件夹
              </Button>
              <Button
                disabled={!selectedResults.length || !!busy}
                onClick={() => void batchExport(true)}
              >
                ZIP 合集
              </Button>
              <Button
                danger
                disabled={!selectedResults.length || !!busy}
                icon={Trash2}
                onClick={() => askDelete("datasets", selectedResults)}
              >
                删除
              </Button>
            </div>
            {noticeBox}
            <div className="history-list">
              {ws?.datasets.map((ds) => (
                <div className="history-row" key={ds.id}>
                  <input
                    type="checkbox"
                    aria-label={`选择结果 ${ds.name}`}
                    checked={selectedResults.includes(ds.id)}
                    onChange={(e) =>
                      setSelectedResults((old) =>
                        e.target.checked
                          ? [...old, ds.id]
                          : old.filter((id) => id !== ds.id),
                      )
                    }
                  />
                  <button
                    className="history-name"
                    onClick={() => {
                      setResultId(ds.id);
                      setQuery(queryFor(ds.id));
                      setTab("result");setPhase("results");
                      setModal(null);
                    }}
                  >
                    <strong>{ds.name}</strong>
                    <small>
                      {ds.summary.outputRows.toLocaleString()} 行 ·{" "}
                      {ds.columns.length} 列 ·{" "}
                      {new Date(ds.createdAt).toLocaleString("zh-CN")}
                    </small>
                  </button>
                  {ds.exports.at(-1) && (
                    <Button
                      icon={FolderOpen}
                      aria-label={`打开 ${ds.name} 所在目录`}
                      onClick={() =>
                        void call("open_path", {
                          path: ds.exports.at(-1),
                          select: true,
                        }).catch(error)
                      }
                    />
                  )}
                </div>
              ))}
            </div>
            {!ws?.datasets.length && (
              <p className="empty-note">
                还没有汇总表。合并完成后会自动保存在这里。
              </p>
            )}
          </div>
          <div className="modal-footer">
            <Button primary onClick={() => setModal(null)}>
              返回工作台
            </Button>
          </div>
        </Modal>
      )}
      {modal === "tasks" && (
        <Modal title="任务记录" wide onClose={() => setModal(null)}>
          <div className="modal-body">
            {ws?.tasks.map((task) => (
              <div className="task-row" key={task.id}>
                <span className={`task-state ${task.state}`}>
                  {taskNames[task.state]}
                </span>
                <div>
                  <strong>{task.phase}</strong>
                  <small>
                    {new Date(task.createdAt).toLocaleString("zh-CN")}
                    {task.error ? ` · ${task.error}` : ""}
                  </small>
                </div>
                {["pending", "running", "cancelling"].includes(task.state) ? (
                  <Button
                    disabled={task.state === "cancelling"}
                    onClick={() =>
                      void call("cancel_task", { taskId: task.id }).catch(error)
                    }
                  >
                    取消
                  </Button>
                ) : ["failed", "cancelled", "interrupted"].includes(
                    task.state,
                  ) ? (
                  <Button
                    disabled={!!busy}
                    onClick={() =>
                      void action("重试任务", async () => {
                        await call("retry_task", { taskId: task.id });
                        await refresh();
                      })
                    }
                  >
                    重试
                  </Button>
                ) : (
                  <Button
                    onClick={() => {
                      setResultId(task.datasetId);
                      setQuery(queryFor(task.datasetId!));
                      setTab("result");setPhase("results");
                      setModal(null);
                    }}
                  >
                    查看结果
                  </Button>
                )}
              </div>
            ))}
            {!ws?.tasks.length && (
              <p className="empty-note">还没有任务记录。</p>
            )}
            {noticeBox}
          </div>
          <div className="modal-footer">
            <Button onClick={() => setModal(null)}>关闭</Button>
          </div>
        </Modal>
      )}
      {modal === "issues" && (
        <Modal title="异常定位 · 保留原值" wide onClose={() => setModal(null)}>
          <div className="modal-body">
            <p>每页显示 200 条，可点击定位到源工作表。</p>
            <div className="issues-table">
              <table>
                <thead>
                  <tr>
                    <th>文件 / 工作表</th>
                    <th>位置</th>
                    <th>原值</th>
                    <th>说明</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {issues.map((issue, i) => (
                    <tr key={i}>
                      <td>
                        {issue.fileName ||
                          ws?.files.find((f) => f.id === issue.fileId)?.name}
                        <small>
                          {
                            ws?.files
                              .flatMap((f) => f.sheets)
                              .find((s) => s.id === issue.sheetId)?.name
                          }
                        </small>
                      </td>
                      <td>
                        第 {issue.row} 行 / {issue.column}
                      </td>
                      <td>{issue.originalValue}</td>
                      <td>{issue.message}</td>
                      <td>
                        <Button
                          onClick={() => {
                            setActiveSheet(issue.sheetId);
                            setFocusRow(issue.row);
                            setTab("source");
                            setModal(null);
                            setNotice({
                              message: `异常位于第 ${issue.row} 行，列「${issue.column}」，可在源工作表查看。`,
                              error: false,
                            });
                          }}
                        >
                          定位
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="toolbar-group">
              <Button
                disabled={issueOffset === 0 || !!busy}
                onClick={() =>
                  dataset &&
                  void showIssues(dataset, Math.max(0, issueOffset - 200))
                }
              >
                上一页
              </Button>
              <span>第 {Math.floor(issueOffset / 200) + 1} 页</span>
              <Button
                disabled={issues.length < 200 || !!busy}
                onClick={() =>
                  dataset && void showIssues(dataset, issueOffset + 200)
                }
              >
                下一页
              </Button>
            </div>
          </div>
          <div className="modal-footer">
            <Button onClick={() => setModal(null)}>关闭</Button>
          </div>
        </Modal>
      )}
      {modal === "settings" && (
        <Modal
          title={settingsTab === "about" ? "关于" : "设置与本地数据"}
          wide
          onClose={() => setModal(null)}
        >
          <div className="settings-nav-tabs">
            <button
              type="button"
              className={`settings-tab-btn ${settingsTab === "general" ? "active" : ""}`}
              onClick={() => setSettingsTab("general")}
            >
              <SlidersHorizontal size={14} />
              <span>设置与数据</span>
            </button>
            <button
              type="button"
              className={`settings-tab-btn ${settingsTab === "about" ? "active" : ""}`}
              onClick={() => setSettingsTab("about")}
            >
              <Info size={14} />
              <span>关于</span>
            </button>
          </div>

          {settingsTab === "general" ? (
            <>
              <div className="modal-body settings-body">
                <section className="settings-card">
                  <div className="settings-card-header">
                    <h3>数据位置</h3>
                    <span className="settings-badge">
                      当前占用 <b>{bytes(disk)}</b> · 保留到手动清理
                    </span>
                  </div>
                  <div className="settings-path-box">
                    <code>{ws?.dataDirectory}</code>
                    <div className="path-actions">
                      <Button
                        className="quiet"
                        icon={Copy}
                        title="复制路径"
                        onClick={() => {
                          if (ws?.dataDirectory) {
                            navigator.clipboard.writeText(ws.dataDirectory);
                            setNotice({ message: "数据目录路径已复制到剪贴板", error: false });
                          }
                        }}
                      >
                        复制
                      </Button>
                      <Button
                        icon={FolderOpen}
                        onClick={() =>
                          void call("open_path", {
                            path: ws?.dataDirectory,
                            select: false,
                          }).catch(error)
                        }
                      >
                        打开目录
                      </Button>
                    </div>
                  </div>
                  <div className="settings-card-actions">
                    <Button icon={History} onClick={() => setModal("activity")}>
                      查看操作记录
                    </Button>
                  </div>
                  <p className="muted">
                    导入的是独立副本，旧版和原始文件不会被修改。删除记录后的数据库空间会用于后续数据。
                  </p>
                </section>

                <section className="settings-card">
                  <div className="settings-card-header">
                    <h3>导入以前的设置和结果</h3>
                  </div>
                  <div className="toolbar-group">
                    <Button
                      icon={Upload}
                      disabled={!!busy}
                      onClick={() =>
                        void action("导入配置", async () => {
                          const files = await chooseFiles(false, true);
                          if (!files.length) return;
                          for (const path of files) {
                            const scheme = await call<Scheme>("import_scheme", {
                              path,
                            });
                            setSchemeId(scheme.id);
                          }
                          await refresh();
                          setNotice({
                            message:
                              "以前的设置已导入。添加新文件后，可在「配置文件」中选择；使用时会检查表名是否对应。",
                            error: false,
                          });
                        })
                      }
                    >
                      导入旧 JSON 配置
                    </Button>
                    <Button
                      icon={History}
                      disabled={!!busy}
                      onClick={() =>
                        void action("导入历史结果", async () => {
                          const paths = await chooseFiles();
                          if (!paths.length) return;
                          await call("import_results", { paths });
                          await refresh();
                          setNotice({
                            message: "历史结果已复制到独立工作区。",
                            error: false,
                          });
                        })
                      }
                    >
                      导入历史结果
                    </Button>
                  </div>
                  {ws?.schemes && ws.schemes.length > 0 ? (
                    <div className="schemes-list">
                      {ws.schemes.map((s) => (
                        <div className="scheme-row" key={s.id}>
                          <strong>{s.name}</strong>
                          <Button
                            icon={Download}
                            aria-label={`导出方案 ${s.name}`}
                            disabled={!!busy}
                            onClick={() =>
                              void action("导出方案", async () => {
                                const path = await savePath("汇总方案.json", "json");
                                if (path)
                                  await call("export_scheme", { schemeId: s.id, path });
                              })
                            }
                          />
                          <Button
                            danger
                            icon={Trash2}
                            aria-label={`删除方案 ${s.name}`}
                            onClick={() => askDelete("schemes", [s.id])}
                          />
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="empty-schemes-note">暂无已保存的配置方案，可从顶部「配置文件」中保存常用规则。</p>
                  )}
                </section>

                <section className="settings-card">
                  <div className="settings-card-header">
                    <h3>可选启动密码</h3>
                  </div>
                  <p className="muted">
                    默认直接进入。设置密码后，下次启动需要解锁。此设置用于保护应用入口，数据文件仍存放在本机。
                  </p>
                  <div className="password-inputs">
                    <label>
                      当前密码
                      <input
                        type="password"
                        autoComplete="current-password"
                        value={currentPassword}
                        onChange={(e) => setCurrentPassword(e.target.value)}
                      />
                    </label>
                    <label>
                      新密码（至少 6 位，留空关闭）
                      <input
                        type="password"
                        autoComplete="new-password"
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                      />
                    </label>
                  </div>
                  <div className="toolbar-group">
                    <Button
                      disabled={!!busy}
                      onClick={() =>
                        void action("修改密码", async () => {
                          await call("set_password", { password, currentPassword });
                          setPassword("");
                          setCurrentPassword("");
                          await refresh();
                          setNotice({
                            message: password
                              ? "启动密码已设置。"
                              : "启动密码已关闭。",
                            error: false,
                          });
                        })
                      }
                    >
                      保存密码设置
                    </Button>
                    <Button
                      icon={LockKeyhole}
                      disabled={!ws?.settings.passwordHash}
                      onClick={() =>
                        void action("锁定", async () => {
                          await call("lock");
                          setModal(null);
                          await refresh(true);
                        })
                      }
                    >
                      立即锁定
                    </Button>
                  </div>
                </section>
                {noticeBox}
              </div>
              <div className="modal-footer">
                <Button primary onClick={() => setModal(null)}>
                  完成
                </Button>
              </div>
            </>
          ) : (
            <>
              <div className="modal-body about-body">
                <div className="about-app-header">
                  <div className="about-app-icon" aria-hidden="true">
                    <img src={appIcon} alt="汇格 · GridFlow" className="about-app-icon-img" />
                  </div>
                  <div className="about-app-titles">
                    <h2>汇格 · GridFlow</h2>
                    <p>本地 Excel / CSV 表格合并与数据分析工作台</p>
                  </div>
                </div>

                <div className="about-info-card">
                  <div className="about-info-row">
                    <span className="about-info-label">当前版本</span>
                    <span className="about-info-value">v2.0.0</span>
                  </div>
                  <div className="about-info-row">
                    <span className="about-info-label">项目地址</span>
                    <a
                      className="about-info-link"
                      href="https://github.com/d8349565/GridFlow"
                      target="_blank"
                      rel="noopener noreferrer"
                      title="打开 GitHub 仓库"
                      onClick={(e) => {
                        e.preventDefault();
                        void call("open_path", {
                          path: "https://github.com/d8349565/GridFlow",
                          select: false,
                        }).catch(() => {
                          window.open("https://github.com/d8349565/GridFlow", "_blank");
                        });
                      }}
                    >
                      <svg className="github-svg" viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                        <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
                      </svg>
                      <span>https://github.com/d8349565/GridFlow</span>
                      <ExternalLink size={13} />
                    </a>
                  </div>
                </div>

                <div className="about-sponsor-section">
                  <div className="about-sponsor-header">
                    <Heart size={16} className="heart-icon" />
                    <h3>赞赏项目</h3>
                  </div>
                  <p className="about-sponsor-desc">
                    如果这个工作台帮到了你，欢迎支持项目的持续维护。赞赏完全自愿。
                  </p>

                  <div className="about-qr-card">
                    <h4>微信赞赏</h4>
                    <div className="about-qr-img-wrapper">
                      <img
                        src={rewardQrCode}
                        alt="微信赞赏码"
                        className="about-qr-image"
                      />
                    </div>
                    <p className="about-qr-caption">使用微信扫一扫</p>
                  </div>
                </div>
              </div>
              <div className="modal-footer about-modal-footer">
                <button
                  type="button"
                  className="about-close-btn"
                  onClick={() => setModal(null)}
                >
                  关闭
                </button>
              </div>
            </>
          )}
        </Modal>
      )}

      {modal === "activity" && (
        <Modal title="本地操作记录" wide onClose={() => setModal(null)}>
          <div className="modal-body">
            {ws?.activities?.map((item) => (
              <div className="activity-row" key={item.id}>
                <strong>{item.action}</strong>
                <span>{item.subject}</span>
                <small>
                  {new Date(item.timestamp).toLocaleString("zh-CN")}
                </small>
              </div>
            ))}
            {!ws?.activities?.length && <p>还没有操作记录。</p>}
          </div>
          <div className="modal-footer">
            <Button onClick={() => setModal(null)}>关闭</Button>
          </div>
        </Modal>
      )}
      {modal === "customSort" && (
        <Modal title="自定义分类顺序" onClose={() => setModal(null)}>
          <div className="modal-body">
            <p>点击上下按钮排列类别；未列出的类别保持默认顺序追加。</p>
            {customValues.map((value, i) => (
              <div className="custom-order-row" key={value}>
                <span>{value || "(空白)"}</span>
                <Button
                  aria-label={`${value} 上移`}
                  disabled={!i}
                  icon={ArrowUp}
                  onClick={() =>
                    setCustomValues((old) => {
                      const next = [...old];
                      [next[i - 1], next[i]] = [next[i], next[i - 1]];
                      return next;
                    })
                  }
                />
                <Button
                  aria-label={`${value} 下移`}
                  disabled={i === customValues.length - 1}
                  icon={ArrowDown}
                  onClick={() =>
                    setCustomValues((old) => {
                      const next = [...old];
                      [next[i], next[i + 1]] = [next[i + 1], next[i]];
                      return next;
                    })
                  }
                />
              </div>
            ))}
          </div>
          <div className="modal-footer">
            <Button onClick={() => setModal(null)}>取消</Button>
            <Button
              primary
              onClick={() => {
                setPivot((p) => ({
                  ...p,
                  customOrder: customValues,
                  sort: null,
                }));
                setModal(null);
              }}
            >
              应用顺序
            </Button>
          </div>
        </Modal>
      )}
      {confirm && (
        <Modal title="确认删除" onClose={() => setConfirm(null)}>
          <div className="modal-body">
            <p>{confirm.message}</p>
            {noticeBox}
          </div>
          <div className="modal-footer">
            <Button onClick={() => setConfirm(null)}>取消</Button>
            <Button
              danger
              loading={busy === "删除"}
              onClick={() => void action("删除", confirm.action)}
            >
              确认删除
            </Button>
          </div>
        </Modal>
      )}
      {modal === "help" && (
        <HelpModal
          onClose={() => setModal(null)}
          onStartTour={() => {
            setPanels((p) => ({ ...p, files: true }));
            setTourStep(0);
            setTourActive(true);
          }}
          onLoadSample={() => void loadSample()}
          hasFiles={!!ws?.files.length}
          busy={!!busy}
        />
      )}
      <OnboardingTour
        active={tourActive}
        step={tourStep}
        onStepChange={setTourStep}
        onClose={() => {
          setTourActive(false);
          try {
            localStorage.setItem("gridflow_tour_completed", "true");
          } catch {}
        }}
        onComplete={() => {
          setTourActive(false);
          try {
            localStorage.setItem("gridflow_tour_completed", "true");
          } catch {}
          setNotice({
            message: "🎉 新手实操引导完成！现在试着框选表头或点击「预览结果」吧。",
            error: false,
          });
        }}
      />
    </div>
  );
}
