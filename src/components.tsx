import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  type ReactNode,
} from "react";
import { AgGridReact } from "ag-grid-react";
import {
  AllCommunityModule,
  ModuleRegistry,
  themeQuartz,
  type ColDef,
  type GridApi,
  type IDatasource,
  type CellClickedEvent,
  type ColumnMovedEvent,
} from "ag-grid-community";
import { X, LoaderCircle } from "lucide-react";
import { cellDisplay, cellText } from "./api";
import type { Cell, Column, DataPage } from "./types";

ModuleRegistry.registerModules([AllCommunityModule]);
export const gridTheme = themeQuartz.withParams({
  accentColor: "#2563eb",
  backgroundColor: "#fff",
  headerBackgroundColor: "#f8fafc",
  borderColor: "#e6ebf2",
  rowBorder: { color: "#eef1f5" },
  fontFamily: "Segoe UI, Microsoft YaHei, sans-serif",
  fontSize: 13,
  headerFontSize: 12,
  cellHorizontalPadding: 14,
  wrapperBorder: false,
  rowHeight: 36,
  headerHeight: 38,
});
export function Button({
  children,
  icon: Icon,
  primary = false,
  danger = false,
  loading = false,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  icon?: React.ComponentType<{ size?: number }>;
  primary?: boolean;
  danger?: boolean;
  loading?: boolean;
}) {
  return (
    <button
      type="button"
      {...props}
      disabled={props.disabled || loading}
      className={`button ${primary ? "primary" : ""} ${danger ? "danger" : ""} ${props.className || ""}`}
    >
      {loading ? (
        <LoaderCircle size={15} className="spin" />
      ) : Icon ? (
        <Icon size={15} />
      ) : null}
      {children}
    </button>
  );
}
export function Modal({
  title,
  children,
  onClose,
  wide = false,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const returnFocus = useRef(document.activeElement as HTMLElement);
  useLayoutEffect(() => {
    ref.current?.showModal();
    return () => {
      ref.current?.close();
      queueMicrotask(() => {
        if (returnFocus.current?.isConnected) returnFocus.current.focus();
      });
    };
  }, []);
  return (
    <dialog
      ref={ref}
      className={wide ? "modal wide" : "modal"}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div className="modal-head">
        <h2>{title}</h2>
        <Button aria-label="关闭弹窗" icon={X} onClick={onClose} />
      </div>
      {children}
    </dialog>
  );
}

export type GridRow = { _number: number; _cells: Record<string, Cell> };
function gridRows(page: DataPage): GridRow[] {
  return page.rows.map((cells, i) => ({
    _number: page.rowNumbers[i] ?? page.offset + i + 1,
    _cells: Object.fromEntries(page.columns.map((c, j) => [c.key, cells[j]])),
  }));
}
export function Grid({
  columns,
  load,
  onCell,
  onMove,
  headerRow,
  version,
  focusRow,
  totalRows,
}: {
  columns: Column[];
  load: (
    offset: number,
    limit: number,
    sorts: { key: string; descending: boolean }[],
  ) => Promise<DataPage>;
  onCell?: (row: number, column: number, value: Cell) => void;
  onMove?: (keys: string[]) => void;
  headerRow?: number;
  version: string;
  focusRow?: number;
  totalRows?: number;
}) {
  const grid = useRef<AgGridReact<GridRow>>(null);
  const latest = useRef(load);
  latest.current = load;
  const epoch = useRef(0);
  const defs = useMemo<ColDef<GridRow>[]>(
    () => [
      {
        colId: "__workbench_row",
        field: "_number",
        headerName: "#",
        width: 65,
        minWidth: 55,
        maxWidth: 90,
        pinned: "left",
        sortable: false,
        lockPosition: true,
        cellClass: "row-number",
      },
      ...columns.map<ColDef<GridRow>>((c) => ({
        colId: "field:" + c.key,
        headerName: c.name,
        valueGetter: (p) => p.data?._cells[c.key],
        minWidth: 100,
        initialWidth: 160,
        sortable: true,
        resizable: true,
        valueFormatter: (p) => cellDisplay(p.value as Cell | undefined),
        tooltipValueGetter: (p) => cellText(p.value as Cell | undefined),
        cellClass: (p) =>
          (p.value as Cell | undefined)?.kind === "number" ? "numeric" : "",
        headerTooltip: c.name,
      })),
    ],
    [JSON.stringify(columns)],
  );
  function position(api: GridApi<GridRow>) {
    if (totalRows) api.setRowCount(totalRows, true);
    if (focusRow) api.ensureIndexVisible(focusRow - 1, "middle");
  }
  function install(api: GridApi<GridRow>) {
    const tag = ++epoch.current;
    const datasource: IDatasource = {
      getRows: (p) =>
        latest
          .current(
            p.startRow,
            p.endRow - p.startRow,
            p.sortModel.map((s) => ({
              key: s.colId.slice(6),
              descending: s.sort === "desc",
            })),
          )
          .then((page) => {
            if (tag !== epoch.current) {
              p.failCallback();
              return;
            }
            p.successCallback(gridRows(page), page.total);
          })
          .catch(() => p.failCallback()),
    };
    api.setGridOption("datasource", datasource);
    position(api);
  }
  useEffect(() => {
    if (grid.current?.api) install(grid.current.api);
  }, [version]);
  useEffect(() => {
    if (grid.current?.api) position(grid.current.api);
  }, [focusRow, totalRows]);
  return (
    <div className="data-grid">
      <AgGridReact<GridRow>
        ref={grid}
        theme={gridTheme}
        columnDefs={defs}
        rowModelType="infinite"
        cacheBlockSize={100}
        maxBlocksInCache={6}
        blockLoadDebounceMillis={60}
        maxConcurrentDatasourceRequests={1}
        animateRows={false}
        enableCellTextSelection
        tooltipShowDelay={450}
        overlayNoRowsTemplate="暂无可显示的数据"
        getRowStyle={(p) =>
          p.data?._number === headerRow
            ? { background: "#eff6ff", fontWeight: 600 }
            : undefined
        }
        onGridReady={(e) => install(e.api)}
        onCellClicked={(e) => {
          if (!e.data || e.colDef.colId === "__workbench_row") return;
          const key = e.colDef.colId?.slice(6);
          const i = columns.findIndex((c) => c.key === key);
          if (i >= 0) onCell?.(e.data._number, i, e.value as Cell);
        }}
        onColumnMoved={(e) => {
          if (e.finished && e.source === "uiColumnMoved")
            onMove?.(
              e.api
                .getColumnState()
                .filter((c) => c.colId.startsWith("field:"))
                .map((c) => c.colId.slice(6)),
            );
        }}
      />
    </div>
  );
}
export function StaticGrid({
  columns,
  rows,
}: {
  columns: Column[];
  rows: Cell[][];
}) {
  const defs = useMemo<ColDef<GridRow>[]>(
    () =>
      columns.map((c) => ({
        colId: "field:" + c.key,
        headerName: c.name,
        valueGetter: (p) => p.data?._cells[c.key],
        initialWidth: 170,
        minWidth: 110,
        sortable: true,
        resizable: true,
        valueFormatter: (p) => cellDisplay(p.value as Cell),
        cellClass: c.dataType === "number" ? "numeric" : "",
      })),
    [JSON.stringify(columns)],
  );
  return (
    <div className="data-grid">
      <AgGridReact<GridRow>
        theme={gridTheme}
        columnDefs={defs}
        rowData={gridRows({
          columns,
          rows,
          rowNumbers: rows.map((_, i) => i + 1),
          total: rows.length,
          offset: 0,
          sample: false,
        })}
        enableCellTextSelection
      />
    </div>
  );
}
