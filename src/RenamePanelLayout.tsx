import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent, type ReactNode } from "react";

type Side = "files" | "rules";
type Widths = Record<Side,number>;
const defaults:Widths={files:232,rules:392}, minimum:Widths={files:180,rules:300}, maximum:Widths={files:520,rules:620};
const preferenceKey="gridflow_rename_panel_widths_v1", dividerWidth=8, minimumPreview=320;
const clamp=(n:number,min:number,max:number) => Math.min(Math.max(n,min),Math.max(min,max));
function initialWidths():Widths {
  try { const saved=JSON.parse(localStorage.getItem(preferenceKey) || "null");
    return {files:Number.isFinite(saved?.files) ? clamp(saved.files,minimum.files,maximum.files) : defaults.files,rules:Number.isFinite(saved?.rules) ? clamp(saved.rules,minimum.rules,maximum.rules) : defaults.rules};
  } catch { return defaults; }
}
function fitted(value:Widths,width:number):Widths {
  const available=width-minimumPreview-dividerWidth*2;
  const files=clamp(value.files,minimum.files,Math.min(maximum.files,available-minimum.rules));
  return {files,rules:clamp(value.rules,minimum.rules,Math.min(maximum.rules,available-files))};
}

export default function RenamePanelLayout({ files, source, rules }: {files:ReactNode;source:ReactNode;rules:ReactNode}) {
  const layout=useRef<HTMLDivElement>(null);
  const [preferred,setPreferred]=useState(initialWidths);
  const [width,setWidth]=useState(window.innerWidth);
  const [resizing,setResizing]=useState<Side | null>(null);
  const drag=useRef<{side:Side;x:number;widths:Widths} | null>(null);
  const pendingX=useRef(0), frame=useRef<number | null>(null), latest=useRef(preferred);
  const actual=fitted(preferred,width);
  function update(side:Side,requested:number,base:Widths,persist=false) {
    const available=(layout.current?.clientWidth ?? width)-minimumPreview-dividerWidth*2;
    const other=side === "files" ? "rules" : "files";
    const next={...base,[side]:clamp(Math.round(requested),minimum[side],Math.min(maximum[side],available-minimum[other]))};
    next[other]=clamp(base[other],minimum[other],Math.min(maximum[other],available-next[side]));
    latest.current=next; setPreferred(next);
    if(persist) { try { localStorage.setItem(preferenceKey,JSON.stringify(next)); } catch {} }
  }
  function start(side:Side,event:PointerEvent<HTMLDivElement>) {
    if(event.button !== 0) return;
    event.preventDefault(); event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId);
    drag.current={side,x:event.clientX,widths:actual}; pendingX.current=event.clientX; setResizing(side);
  }
  function apply(x:number) { const start=drag.current; if(start) update(start.side,start.widths[start.side]+(x-start.x)*(start.side === "files" ? 1 : -1),start.widths); }
  function move(event:PointerEvent<HTMLDivElement>) {
    if(!drag.current) return; pendingX.current=event.clientX;
    if(frame.current === null) frame.current=requestAnimationFrame(() => { frame.current=null; apply(pendingX.current); });
  }
  function finish() {
    if(!drag.current) return;
    if(frame.current !== null) { cancelAnimationFrame(frame.current); frame.current=null; }
    apply(pendingX.current); drag.current=null; setResizing(null);
    try { localStorage.setItem(preferenceKey,JSON.stringify(latest.current)); } catch {}
  }
  function key(side:Side,event:KeyboardEvent<HTMLDivElement>) {
    const limit=Math.min(maximum[side],width-minimumPreview-dividerWidth*2-minimum[side === "files" ? "rules" : "files"]);
    let next=actual[side],step=event.shiftKey ? 32 : 16;
    if(event.key === "ArrowLeft") next+=side === "files" ? -step : step;
    else if(event.key === "ArrowRight") next+=side === "files" ? step : -step;
    else if(event.key === "Home") next=minimum[side];
    else if(event.key === "End") next=limit;
    else return;
    event.preventDefault(); update(side,next,actual,true);
  }
  useEffect(() => {
    const element=layout.current; if(!element) return;
    setWidth(element.clientWidth);
    const observer=new ResizeObserver(() => setWidth(element.clientWidth)); observer.observe(element);
    return () => { observer.disconnect(); if(frame.current !== null) cancelAnimationFrame(frame.current); };
  },[]);
  function divider(side:Side) {
    const name=side === "files" ? "文件列表" : "命名规则";
    return <div className={`rename-splitter ${resizing === side ? "active" : ""}`} role="separator" aria-orientation="vertical" aria-label={`调整${name}宽度`} aria-controls={side === "files" ? "rename-files-panel" : "rename-rules-panel"} aria-valuemin={minimum[side]} aria-valuemax={Math.max(minimum[side],Math.min(maximum[side],width-minimumPreview-dividerWidth*2-minimum[side === "files" ? "rules" : "files"]))} aria-valuenow={Math.round(actual[side])} aria-valuetext={`${Math.round(actual[side])} 像素`} tabIndex={0} title={`拖动调整${name}宽度；双击恢复默认，左右键可微调`} onPointerDown={e => start(side,e)} onPointerMove={move} onPointerUp={finish} onPointerCancel={finish} onLostPointerCapture={finish} onKeyDown={e => key(side,e)} onDoubleClick={() => update(side,defaults[side],actual,true)} />;
  }
  return <div ref={layout} className={`rename-workspace ${resizing ? "is-resizing" : ""}`} style={{"--rename-file-width":`${actual.files}px`,"--rename-rule-width":`${actual.rules}px`} as CSSProperties}>
    {files}{divider("files")}{source}{divider("rules")}{rules}
  </div>;
}
