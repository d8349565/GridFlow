import { useEffect, useRef, useState, type ReactNode, type PointerEvent } from "react";
import { GripVertical } from "lucide-react";
import type { ColumnRule } from "./types";

export function OutputColumnList({ columns, onMove, children }: {
  columns: ColumnRule[];
  onMove: (key: string, index: number) => void;
  children: (column: ColumnRule, index: number, handle: ReactNode) => ReactNode;
}) {
  const list = useRef<HTMLDivElement>(null);
  const pointer = useRef<{ key: string; id: number; startY: number; x: number; y: number; moved: boolean } | null>(null);
  const [drag, setDrag] = useState<{ key: string; index: number } | null>(null);

  function destination() {
    const current = pointer.current;
    const root = list.current;
    if (!current || !root) return null;
    const bounds = root.getBoundingClientRect();
    const panel = root.closest(".panel-scroll")?.getBoundingClientRect() ?? bounds;
    if (current.x < bounds.left || current.x > bounds.right || current.y < Math.max(bounds.top, panel.top) || current.y > Math.min(bounds.bottom, panel.bottom)) return null;
    return Array.from(root.querySelectorAll<HTMLElement>("[data-output-column]"))
      .filter((row) => row.dataset.outputColumn !== current.key)
      .filter((row) => {
        const rect = row.getBoundingClientRect();
        return current.y > rect.top + rect.height / 2;
      }).length;
  }
  function cancel() {
    pointer.current = null;
    setDrag(null);
  }
  function finish(event: PointerEvent<HTMLButtonElement>) {
    const current = pointer.current;
    if (!current || current.id !== event.pointerId) return;
    current.x = event.clientX;
    current.y = event.clientY;
    const index = destination();
    if (current.moved && index !== null) onMove(current.key, index);
    cancel();
  }
  useEffect(() => {
    if (!drag) return;
    let frame: number;
    function tick() {
      const current = pointer.current;
      if (!current) return;
      if (current.moved) {
        const panel = list.current?.closest<HTMLElement>(".panel-scroll");
        if (panel) {
          const bounds = panel.getBoundingClientRect();
          if (current.x >= bounds.left && current.x <= bounds.right && current.y >= bounds.top && current.y <= bounds.bottom) {
            const speed = current.y < bounds.top + 32 ? -8 : current.y > bounds.bottom - 32 ? 8 : 0;
            if (speed) panel.scrollBy(0, speed);
          }
        }
        const index = destination();
        setDrag((old) => old && old.index !== (index ?? -1) ? { ...old, index: index ?? -1 } : old);
      }
      frame = requestAnimationFrame(tick);
    }
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [drag?.key]);

  const remaining = columns.filter((column) => column.key !== drag?.key);
  const before = drag && drag.index >= 0 ? remaining[drag.index]?.key : undefined;
  const after = drag && drag.index === remaining.length ? remaining.at(-1)?.key : undefined;
  return <div ref={list} className="output-column-list" onKeyDown={(event) => {
    if (event.key === "Escape" && pointer.current) { event.preventDefault(); event.stopPropagation(); cancel(); }
  }}>
    {columns.map((column, index) => <div key={column.key} data-output-column={column.key}
      className={`column-rule${drag?.key === column.key ? " column-dragging" : ""}${before === column.key ? " column-drop-before" : ""}${after === column.key ? " column-drop-after" : ""}`}>
      {children(column, index, <button type="button" className="column-drag-handle" title="按住拖动排序；也可用右侧箭头调整" aria-label={`拖动排序 ${column.name}，也可使用上下移动按钮`}
        disabled={columns.length < 2}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          event.currentTarget.setPointerCapture(event.pointerId);
          pointer.current = { key: column.key, id: event.pointerId, startY: event.clientY, x: event.clientX, y: event.clientY, moved: false };
          setDrag({ key: column.key, index });
        }}
        onPointerMove={(event) => {
          const current = pointer.current;
          if (!current || current.id !== event.pointerId) return;
          current.x = event.clientX;
          current.y = event.clientY;
          current.moved ||= Math.abs(current.y - current.startY) >= 4;
        }}
        onPointerUp={finish} onPointerCancel={cancel} onLostPointerCapture={cancel}>
        <GripVertical size={14} aria-hidden="true" />
      </button>)}
    </div>)}
  </div>;
}
