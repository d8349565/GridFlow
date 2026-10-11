import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { call, cellText, coordinate } from "./api";
import type { Cell, DataPage, SheetAppearance, SheetRange, SourceSheet } from "./types";
import { excelText, formatColour } from "./excelFormat";

const ROW = 32, GUTTER = 46, HEAD = 30;
export const rangeLabel = (r: SheetRange) => {
  const start = coordinate(r.startRow,r.startColumn), end = coordinate(r.endRow,r.endColumn);
  return start === end ? start : `${start}:${end}`;
};
export function normalizeRange(a: { row: number; column: number }, b: { row: number; column: number }): SheetRange {
  return { startRow: Math.min(a.row,b.row),endRow: Math.max(a.row,b.row),startColumn: Math.min(a.column,b.column),endColumn: Math.max(a.column,b.column) };
}
function intersects(a: SheetRange,b: SheetRange) { return a.startRow <= b.endRow && a.endRow >= b.startRow && a.startColumn <= b.endColumn && a.endColumn >= b.startColumn; }
function rowAt(offsets: Float64Array, y: number) {
  let low=0,high=offsets.length-1;
  while(low<high) { const mid=Math.floor((low+high+1)/2); if(offsets[mid]<=y)low=mid;else high=mid-1; }
  return Math.min(offsets.length-1,low+1);
}
export default function ExcelPreview({ sheet, selection, header, onSelect, focusRow }: {
  sheet: SourceSheet; selection: SheetRange | null; header: SheetRange;
  onSelect: (range: SheetRange, cell: Cell | null) => void; focusRow?: number;
}) {
  const viewport = useRef<HTMLDivElement>(null);
  const [scroll,setScroll] = useState({ top: 0,left: 0,height: 400 });
  const [pages,setPages] = useState<Record<number,DataPage>>({});
  const [looks,setLooks] = useState<Record<number,SheetAppearance>>({});
  const [loadError,setLoadError] = useState("");
  const [retry,setRetry] = useState(0);
  const [showHidden,setShowHidden] = useState(false);
  const requests = useRef(new Set<number>());
  const alive = useRef(true);
  useEffect(() => { alive.current=true; return () => { alive.current=false; }; },[]);
  const drag = useRef<{ start: { row: number; column: number }; rows: boolean } | null>(null);
  const anchor = useRef<{ row: number; column: number } | null>(null);
  const extent = useRef<{ row: number; column: number } | null>(null);
  const layout=looks[0] ?? Object.values(looks)[0];
  const defaultHeight=layout?.defaultRowHeight || ROW;
  const rowOffsets=useMemo(() => { const a=new Float64Array(sheet.rows+1);for(let r=1;r<=sheet.rows;r++){const h=layout?.rowHeights?.[r]??defaultHeight;a[r]=a[r-1]+(h===0&&showHidden?defaultHeight:h);}return a; },[sheet.rows,layout?.rowHeights,defaultHeight,showHidden]);
  const first = Math.max(1,rowAt(rowOffsets,scroll.top)-3);
  const last = Math.min(sheet.rows,rowAt(rowOffsets,scroll.top+scroll.height)+8);
  const blocks = [...new Set([Math.floor((first-1)/100),Math.floor(Math.max(0,last-1)/100),0,
    ...(selection && selection.startRow===selection.endRow && selection.startColumn===selection.endColumn ? [Math.floor((selection.startRow-1)/100)] : [])])];
  const wanted=useRef(blocks);wanted.current=blocks;
  const appearance = layout;
  const digitWidth=useMemo(() => {
    const s=appearance?.styles[0],canvas=document.createElement("canvas"),ctx=canvas.getContext("2d");if(!ctx)return 7;
    ctx.font=`${(s?.fontSize??11)*4/3}px ${JSON.stringify(s?.fontName || "Calibri")}`;
    return Math.max(1,Math.round(Math.max(..."0123456789".split("").map((v) => ctx.measureText(v).width))));
  },[appearance?.styles[0]?.fontName,appearance?.styles[0]?.fontSize]);
  const widths = (appearance?.widths.length === sheet.width ? appearance.widths : Array(sheet.width).fill(110)).map((w,i) => {
    if(w===0)return showHidden?64:0;const raw=appearance?.columnWidths?.[i+1];
    return raw===undefined?w:Math.max(1,Math.trunc((256*raw+Math.trunc(128/digitWidth))/256*digitWidth));
  });
  const offsets = useMemo(() => { const a = [GUTTER]; for (const w of widths) a.push(a.at(-1)!+w); return a; },[JSON.stringify(widths)]);
  const totalWidth = offsets.at(-1)!;
  const styleIds = useMemo(() => new Map(Object.values(looks).flatMap((v) => v.cells.map((c) => [`${c.row}:${c.column}`,c.style] as const))),[looks]);
  const numbers=useMemo(() => new Map(Object.values(looks).flatMap((v) => v.cells.map((c) => [`${c.row}:${c.column}`,c.rawNumber] as const))),[looks]);
  const data = (row: number,column: number) => pages[Math.floor((row-1)/100)]?.rows[(row-1)%100]?.[column-1];
  const mergeAt = (row: number,column: number) => appearance?.merges.find((m) => row >= m.startRow && row <= m.endRow && column >= m.startColumn && column <= m.endColumn);
  useEffect(() => {
    for (const block of blocks) {
      if (requests.current.has(block) || pages[block]) continue;
      requests.current.add(block);
      Promise.all([call<DataPage>("preview_sheet",{ sheetId: sheet.id,offset: block*100,limit: 100,headerRow: null }),call<SheetAppearance>("preview_appearance",{ sheetId: sheet.id,offset: block*100,limit: 100 })])
        .then(([page,look]) => {
          if (!alive.current) return;
          requests.current.delete(block);
          setPages((old) => { const next = { ...old,[block]: page }; for (const key of Object.keys(next)) if (!wanted.current.includes(Number(key)) && Math.abs(Number(key)-wanted.current[0])>2) delete next[Number(key)]; return next; });
          setLooks((old) => { const next = { ...old,[block]: look }; for (const key of Object.keys(next)) if (!wanted.current.includes(Number(key)) && Math.abs(Number(key)-wanted.current[0])>2) delete next[Number(key)]; return next; });
          setLoadError("");
        }).catch((e) => { requests.current.delete(block); if (alive.current) setLoadError(e.message ?? String(e)); });
    }
  },[sheet.id,blocks.join(","),retry]);
  useEffect(() => { const el = viewport.current; if (!el) return; const resize = new ResizeObserver(() => setScroll((s) => ({ ...s,height: el.clientHeight }))); resize.observe(el); return () => resize.disconnect(); },[]);
  useEffect(() => { if (focusRow && viewport.current) viewport.current.scrollTop = rowOffsets[Math.max(0,focusRow-3)] ?? 0; },[focusRow]);
  useEffect(() => {
    if (selection && selection.startRow===selection.endRow && selection.startColumn===selection.endColumn) {
      const value=data(selection.startRow,selection.startColumn); if (value) onSelect(selection,value);
    }
  },[pages,selection ? rangeLabel(selection) : ""]);
  function select(row: number,column: number,extend = false,wholeRows = false) {
    if (!sheet.rows || !sheet.width) return;
    row = Math.max(1,Math.min(sheet.rows,row)); column = Math.max(1,Math.min(sheet.width,column));
    const merge = mergeAt(row,column);
    if (merge && !wholeRows && !extend) { row = merge.startRow; column = merge.startColumn; }
    if (!extend || !anchor.current) anchor.current = { row,column };
    extent.current = { row,column };
    const r = normalizeRange(anchor.current,{ row,column });
    if (wholeRows) { r.startColumn = 1; r.endColumn = sheet.width; }
    onSelect(r,r.startRow===r.endRow && r.startColumn===r.endColumn ? data(row,column) ?? null : null);
  }
  const active = selection ? coordinate(selection.startRow,selection.startColumn) : "A1";
  const visibleCells: Array<{ row: number; col: number; range: SheetRange }> = [];
  for (let row=first;row<=last;row++) for (let col=1;col<=sheet.width;col++) {
    if (rowOffsets[row]===rowOffsets[row-1] || widths[col-1]===0) continue;
    if (offsets[col] < scroll.left || offsets[col-1] > scroll.left + (viewport.current?.clientWidth ?? 1200) + 100) continue;
    const m = mergeAt(row,col);
    if (m && (m.startRow !== row || m.startColumn !== col)) continue;
    visibleCells.push({ row,col,range: m ?? { startRow: row,endRow: row,startColumn: col,endColumn: col } });
  }
  for (const m of appearance?.merges ?? []) if (m.startRow <= last && m.endRow >= first && m.startColumn <= sheet.width
    && !visibleCells.some((c) => c.row===m.startRow && c.col===m.startColumn))
    visibleCells.push({ row: m.startRow,col: m.startColumn,range: m });
  const visibleRows = Array.from({ length: Math.max(0,last-first+1) },(_,i) => first+i);
  return <div className="excel-preview">
    <div className="excel-formula"><span className="excel-address">{selection ? rangeLabel(selection) : "选择单元格"}</span><span className="excel-fx">原值</span><span>{selection && selection.startRow===selection.endRow && selection.startColumn===selection.endColumn ? cellText(data(selection.startRow,selection.startColumn)) : selection ? `已选 ${selection.endRow-selection.startRow+1} 行、${selection.endColumn-selection.startColumn+1} 列` : "点击单元格选中，拖动或 Shift + 点击选择区域"}</span>
      {(widths.some((w) => w===0)||Object.values(appearance?.rowHeights??{}).some((h) => h===0)||showHidden) && <button type="button" className="text-button" aria-pressed={showHidden} onClick={() => setShowHidden(!showHidden)}>{showHidden?"按原文件隐藏":"显示隐藏行列"}</button>}
    </div>
    {loadError && <div className="notice error" role="alert">读取原表失败：{loadError}<button onClick={() => { requests.current.clear(); setRetry((n) => n+1); }}>重新读取</button></div>}
    <div ref={viewport} className="excel-scroll" role="grid" aria-label={`${sheet.name} Excel 预览`} aria-rowcount={sheet.rows} aria-colcount={sheet.width} aria-activedescendant={`excel-${active}`} tabIndex={0}
      onScroll={(e) => setScroll({ top: e.currentTarget.scrollTop,left: e.currentTarget.scrollLeft,height: e.currentTarget.clientHeight })}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        const target = (e.target as HTMLElement).closest<HTMLElement>("[data-cell],[data-row-pick]"); if (!target) return;
        e.preventDefault(); e.currentTarget.focus();
        const row = Number(target.dataset.row),column = Number(target.dataset.column ?? 1),rows = !!target.dataset.rowPick;
        select(row,column,e.shiftKey,rows); drag.current = { start: anchor.current!,rows }; e.currentTarget.setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => {
        if (!drag.current) return;
        const el=e.currentTarget,b=el.getBoundingClientRect();
        const row=rowAt(rowOffsets,Math.max(0,e.clientY-b.top+el.scrollTop-HEAD));
        const x=e.clientX-b.left+el.scrollLeft;
        const found=offsets.findIndex((v) => v>x),col=found<0?sheet.width:Math.max(1,found);
        select(row,col,true,drag.current.rows);
        if(e.clientY>b.bottom-24) el.scrollTop+=ROW; else if(e.clientY<b.top+HEAD+12) el.scrollTop=Math.max(0,el.scrollTop-ROW);
        if(e.clientX>b.right-24)el.scrollLeft+=32;else if(e.clientX<b.left+GUTTER+12)el.scrollLeft=Math.max(0,el.scrollLeft-32);
      }} onPointerUp={() => { drag.current=null; }} onPointerCancel={() => { drag.current=null; }}
      onKeyDown={(e) => {
        const current=extent.current ?? { row: selection?.startRow ?? 1,column: selection?.startColumn ?? 1 };
        let row=current.row,col=current.column;
        const merged=mergeAt(row,col);
        if (e.key === " " && e.shiftKey) { e.preventDefault(); select(row,1,false,true); return; }
        switch(e.key) { case "Escape":drag.current=null;return;case "ArrowDown": row=(merged?.endRow??row)+1; break; case "ArrowUp": row=(merged?.startRow??row)-1; break; case "ArrowLeft": col=(merged?.startColumn??col)-1; break; case "ArrowRight": col=(merged?.endColumn??col)+1; break; case "Home": col=1;if(e.ctrlKey)row=1;break; case "End":col=sheet.width;if(e.ctrlKey)row=sheet.rows;break;case "PageDown":row+=Math.floor(scroll.height/ROW);break;case "PageUp":row-=Math.floor(scroll.height/ROW);break; default:return; }
        const rowDirection=e.key==="ArrowUp"||e.key==="PageUp"||e.key==="End"?-1:1;
        const columnDirection=e.key==="ArrowLeft"||e.key==="End"?-1:1;
        while(row>=1&&row<=sheet.rows&&rowOffsets[row]===rowOffsets[row-1])row+=rowDirection;
        while(col>=1&&col<=sheet.width&&widths[col-1]===0)col+=columnDirection;
        e.preventDefault(); e.currentTarget.focus(); select(row,col,e.shiftKey);
        row=Math.max(1,Math.min(row,sheet.rows));col=Math.max(1,Math.min(col,sheet.width));
        const el=e.currentTarget; if(rowOffsets[row-1] < el.scrollTop)el.scrollTop=rowOffsets[row-1];else if(rowOffsets[row]>el.scrollTop+el.clientHeight-HEAD)el.scrollTop=rowOffsets[row]-el.clientHeight+HEAD;
        if(offsets[col-1]<el.scrollLeft+GUTTER)el.scrollLeft=offsets[col-1]-GUTTER;else if(offsets[col]>el.scrollLeft+el.clientWidth)el.scrollLeft=offsets[col]-el.clientWidth;
      }}>
      <div className="excel-canvas" style={{ width: totalWidth,minWidth: "100%",height: Math.max(ROW,rowOffsets[sheet.rows])+HEAD }}>
        <div className="excel-column-heads" style={{ width: totalWidth }}><div className="excel-corner" />{widths.map((w,i) => w>0?<span key={i} style={{ left: offsets[i],width: w }}>{coordinate(1,i+1).slice(0,-1)}</span>:null)}</div>
        {[...new Set(visibleCells.map((v) => v.row))].map((r) => <div key={r} role="row" aria-rowindex={r} style={{ display: "contents" }}>{visibleCells.filter((v) => v.row===r).map(({ row,col,range: r }) => {
          const s=appearance?.styles[styleIds.get(`${row}:${col}`) ?? 0],cell=data(row,col),selected=!!selection&&intersects(r,selection);
          const css: CSSProperties = { left: offsets[col-1],top: HEAD+rowOffsets[row-1],width: offsets[Math.min(sheet.width,r.endColumn)]-offsets[col-1],height: rowOffsets[Math.min(sheet.rows,r.endRow)]-rowOffsets[row-1],
            fontFamily:s?.fontName?`${JSON.stringify(s.fontName)}, "Microsoft YaHei", sans-serif`:undefined,fontWeight: s?.bold ? 700 : 400,fontStyle: s?.italic ? "italic" : undefined,fontSize: s?.fontSize ? `${s.fontSize*4/3}px` : undefined,color:formatColour(s?.numberFormat??"",cell) || s?.color || undefined,backgroundColor: s?.background || undefined,textAlign: (s?.align || (cell?.kind==="number"?"right":"left")) as CSSProperties["textAlign"],whiteSpace:s?.wrap?"pre-wrap":"pre", borderLeft:s?.borders[0] || undefined,borderRight:s?.borders[1] || (appearance?.showGridLines===false?"1px solid transparent":undefined),borderTop:s?.borders[2] || undefined,borderBottom:s?.borders[3] || (appearance?.showGridLines===false?"1px solid transparent":undefined),
            alignItems:s?.verticalAlign==="top"?"flex-start":s?.verticalAlign==="center"?"center":"flex-end",textDecoration:[s?.underline?"underline":"",s?.strike?"line-through":""].filter(Boolean).join(" "),paddingLeft:s?.indent?`${s.indent*3+0.3}em`:undefined };
          const value=excelText(cell,s?.numberFormat,numbers.get(`${row}:${col}`),appearance?.date1904);
          const rotation=s?.rotation===255?0:s?.rotation && s.rotation>90?90-s.rotation:s?.rotation??0;
          return <div key={`${row}:${col}`} id={`excel-${coordinate(row,col)}`} data-cell={coordinate(row,col)} data-row={row} data-column={col} className={`excel-cell ${selected?"selected":""} ${intersects(r,header)?"header-range":""}`} role="gridcell" aria-selected={selected} aria-rowindex={row} aria-colindex={col} aria-rowspan={r.endRow-row+1} aria-colspan={r.endColumn-col+1} aria-label={`${coordinate(row,col)} ${value}`} title={`${coordinate(row,col)} ${value}`} style={css}><span style={{width:"100%",transform:rotation?`rotate(${-rotation}deg)`:undefined,writingMode:s?.rotation===255?"vertical-rl":undefined,textOrientation:s?.rotation===255?"upright":undefined}}>{value}</span></div>;
        })}</div>)}
        <div className="excel-row-heads" style={{ height: rowOffsets[sheet.rows] }}>{visibleRows.filter((row) => rowOffsets[row]>rowOffsets[row-1]).map((row) => <button type="button" tabIndex={-1} key={row} data-row-pick="true" data-row={row} aria-label={`选择第 ${row} 行`} className={row>=header.startRow && row<=header.endRow?"is-header":""} style={{ top: HEAD+rowOffsets[row-1],height:rowOffsets[row]-rowOffsets[row-1] }} onClick={(e) => { if(e.detail===0) select(row,1,e.shiftKey,true); }}>{row}</button>)}</div>
      </div>
    </div>
    <div className="excel-caption">{appearance?.note ?? "蓝色标记为已设置的表头 · 点击行号可选整行，拖动或 Shift + 点击可框选多行"}{!pages[blocks[0]] && <span role="status"> · 正在读取…</span>}</div>
  </div>;
}
