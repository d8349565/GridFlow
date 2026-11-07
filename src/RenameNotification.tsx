import { useEffect, useRef, useState } from "react";
import { AlertTriangle, CheckCircle2, CircleAlert, Info, X } from "lucide-react";
import { Button } from "./components";

export type RenameNoticeData = { kind:"success" | "info" | "warning" | "error"; title:string; detail?:string; path?:string };
const icons={success:CheckCircle2,info:Info,warning:AlertTriangle,error:CircleAlert};

export default function RenameNotification({ notice, onClose, onRetry, paused=false }: {
  notice:RenameNoticeData; onClose?:() => void; onRetry?:() => void; paused?:boolean;
}) {
  const [hovered,setHovered]=useState(false);
  const [focused,setFocused]=useState(false);
  const close=useRef(onClose);close.current=onClose;
  useEffect(() => {
    if(paused || hovered || focused || notice.kind === "error" || notice.kind === "warning" || !close.current) return;
    const timer=setTimeout(() => close.current?.(),5000); return () => clearTimeout(timer);
  },[notice,hovered,focused,paused]);
  const Icon=icons[notice.kind];
  const persistent=notice.kind === "error" || notice.kind === "warning";
  return <div className={`rename-toast ${notice.kind}`} role={persistent ? "alert" : "status"} aria-atomic="true"
    onPointerEnter={() => setHovered(true)} onPointerLeave={() => setHovered(false)}
    onFocusCapture={() => setFocused(true)} onBlurCapture={e => { if(!e.currentTarget.contains(e.relatedTarget as Node | null)) setFocused(false); }}>
    <Icon className="rename-toast-icon" size={20} aria-hidden="true" />
    <div className="rename-toast-body"><strong>{notice.title}</strong>
      {notice.detail && <p className={notice.path ? "rename-toast-file" : ""} title={notice.path}>{notice.detail}</p>}
      {onRetry && <Button className="quiet" onClick={onRetry}>重试读取</Button>}
    </div>
    {onClose && <button type="button" className="rename-toast-close" aria-label="关闭提示" title="关闭提示" onClick={onClose}><X size={15} aria-hidden="true" /></button>}
  </div>;
}
