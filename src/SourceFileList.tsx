import { useEffect, useState } from "react";
import { ChevronRight, Trash2 } from "lucide-react";
import { bytes } from "./api";
import type { SourceFile, SourceSelection, SourceSheet } from "./types";

export default function SourceFileList({ files, sources, activeSheet, onSelect, onPreview, onDelete }: {
  files: SourceFile[];
  sources: SourceSelection[];
  activeSheet: string | null;
  onSelect: (sheetIds: string[], enabled: boolean) => void;
  onPreview: (sheetId: string) => void;
  onDelete: (fileId: string) => void;
}) {
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const selections = new Map(sources.map((source) => [source.sheetId, source]));
  const rows = (sheet: SourceSheet) => Math.max(0, sheet.rows - (selections.get(sheet.id)?.headerRow ?? 1));
  const activeFile = files.find((file) => file.sheets.some((sheet) => sheet.id === activeSheet));
  useEffect(() => {
    if (!activeFile || activeFile.sheets.length < 2) return;
    setExpanded((old) => old[activeFile.id] ? old : { ...old, [activeFile.id]: true });
  }, [activeFile?.id, activeSheet]);

  const first = files[0]?.sheets[0];
  const commonName = first && files.every((file) => file.sheets.length === 1 && file.sheets[0].name === first.name) ? first.name : "";
  const commonShape = commonName && first && files.every((file) => rows(file.sheets[0]) === rows(first) && file.sheets[0].width === first.width);
  const stem = (name: string) => name.replace(/\.(xlsx?|csv)$/i, "");
  const uniqueStems = new Set(files.map((file) => stem(file.name))).size === files.length;
  const prefix = `${commonName}_`;
  const hidePrefix = !!commonName && files.length > 1 && files.every((file) => stem(file.name).startsWith(prefix) && stem(file.name).length > prefix.length);
  const sheetInfo = (sheet: SourceSheet) => `${sheet.name} · ${rows(sheet).toLocaleString()} 条 · ${sheet.width} 列`;

  return <>
    {commonName && <div className="source-file-caption" title="这些文件的工作表名称相同；每份文件仍可单独勾选">
      <span>{commonName}</span>
      {commonShape && first && <small>每表 {rows(first).toLocaleString()} 条 · {first.width} 列</small>}
    </div>}
    <div className="file-scroll">
      {files.map((file) => {
        const single = file.sheets.length === 1;
        const selected = file.sheets.filter((sheet) => selections.get(sheet.id)?.enabled).length;
        const active = file.id === activeFile?.id;
        const open = expanded[file.id] ?? active;
        const baseName = uniqueStems ? stem(file.name) : file.name;
        const label = hidePrefix ? baseName.slice(prefix.length) : baseName;
        const info = `${file.name}\n${file.sheets.map(sheetInfo).join("\n")}\n${bytes(file.size)}${file.encoding ? ` · ${file.encoding}` : ""}`;
        const descriptionId = `file-info-${file.id}`;
        return <div className="source-file" key={file.id}>
          <div className={`source-file-row ${single ? "sheet-item" : ""} ${active ? "active" : ""}`}>
            <label className="source-select" title={single ? "勾选此表参与汇总" : "勾选此文件中的全部工作表"}>
              <input type="checkbox" aria-label={single ? `选择 ${file.name} / ${file.sheets[0].name}` : `选择 ${file.name} 的全部工作表`}
                checked={selected > 0 && selected === file.sheets.length}
                ref={(input) => { if (input) input.indeterminate = selected > 0 && selected < file.sheets.length; }}
                onChange={(event) => onSelect(file.sheets.map((sheet) => sheet.id), event.target.checked)} />
            </label>
            {!single && <button className={`source-expand ${open ? "open" : ""}`} aria-label={`${open ? "收起" : "展开"} ${file.name}`} aria-expanded={open}
              onClick={() => setExpanded((old) => ({ ...old, [file.id]: !open }))}><ChevronRight size={14} /></button>}
            <button className="source-file-preview" title={info} aria-label={single ? `预览 ${file.name} / ${file.sheets[0].name}` : `预览 ${file.name}`} aria-describedby={descriptionId} aria-current={active ? "true" : undefined}
              onClick={() => { if (!single) setExpanded((old) => ({ ...old, [file.id]: true }));if (file.sheets[0]) onPreview(file.sheets[0].id); }}>
              <span>{label}</span>
            </button>
            {!single && <small className="source-sheet-count" title={`${file.sheets.length} 张工作表`}>{file.sheets.length}</small>}
            <button className="source-file-delete" title="删除导入副本" aria-label={`删除 ${file.name}`} onClick={() => onDelete(file.id)}><Trash2 size={13} /></button>
            <span id={descriptionId} className="visually-hidden">{info}</span>
          </div>
          {!single && open && <div className="source-file-sheets">
            {file.sheets.map((sheet) => <div className={`source-sheet-row sheet-item ${activeSheet === sheet.id ? "active" : ""}`} key={sheet.id}>
              <label className="source-select"><input type="checkbox" aria-label={`选择 ${file.name} / ${sheet.name}`} checked={selections.get(sheet.id)?.enabled ?? false}
                onChange={(event) => onSelect([sheet.id], event.target.checked)} /></label>
              <button className="source-file-preview" title={sheetInfo(sheet)} aria-label={`预览 ${file.name} / ${sheet.name}`} aria-current={activeSheet === sheet.id ? "true" : undefined} onClick={() => onPreview(sheet.id)}><span>{sheet.name}</span></button>
            </div>)}
          </div>}
        </div>;
      })}
    </div>
  </>;
}
