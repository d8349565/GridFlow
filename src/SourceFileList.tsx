import { useEffect, useState } from "react";
import { ChevronRight, Settings2, Trash2 } from "lucide-react";
import { bytes } from "./api";
import type { FileSelection, SourceFile, SourceSelection, SourceSheet } from "./types";

function SheetRow({ file, sheet, source, enabled, disabled, active, onChange, onPreview }: {
  file: SourceFile; sheet: SourceSheet; source?: SourceSelection; enabled: boolean; disabled: boolean; active: boolean;
  onChange: (id: string, patch: Partial<SourceSelection>) => void; onPreview: (id: string) => void;
}) {
  const [header, setHeader] = useState(String(source?.headerRow ?? 1));
  const [error, setError] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const showSettings = settingsOpen || !!error;
  useEffect(() => { setHeader(String(source?.headerRow ?? 1)); setError(""); }, [source?.headerRow]);
  function commitHeader() {
    const row = Number(header);
    if (!header || !Number.isInteger(row) || row < 1 || row > sheet.rows) { setError(`行号需为 1～${sheet.rows || 1}`); return; }
    onChange(sheet.id, { headerRow: row, headerRange: undefined }); setHeader(String(row)); setError("");
  }
  return <div className={`source-sheet-config ${active ? "active" : ""} ${!enabled ? "file-disabled" : ""}`}>
    <div className="source-sheet-row sheet-item">
      <label className="source-select"><input type="checkbox" aria-label={`汇总 ${file.name} / ${sheet.name}`} checked={source?.enabled ?? false} disabled={disabled || !enabled || !sheet.width || !sheet.rows}
        onChange={(e) => onChange(sheet.id, { enabled: e.target.checked })} /></label>
      <button className="source-file-preview" title={sheet.name} aria-current={active ? "true" : undefined} onClick={() => onPreview(sheet.id)}><span>{sheet.name}</span></button>
      <button className="text-button source-preview-link" onClick={() => onPreview(sheet.id)}>预览</button>
      <button className="source-sheet-settings-toggle" title="表头与来源设置" aria-label={`${file.name} / ${sheet.name} 表头与来源设置`} aria-expanded={showSettings} aria-controls={`settings-${sheet.id}`} onClick={() => setSettingsOpen((open) => !open)}><Settings2 size={14} aria-hidden="true" /></button>
    </div>
    <div className="source-sheet-settings" id={`settings-${sheet.id}`} hidden={!showSettings}>
      <div className="source-sheet-fields">
        <label>表头行<input type="number" required aria-label={`${file.name} / ${sheet.name} 表头行`} min={1} max={sheet.rows || 1} disabled={disabled || !enabled || !source?.enabled || !sheet.rows} value={header} aria-invalid={!!error} aria-describedby={error ? `header-${sheet.id}` : undefined}
          onChange={(e) => { setHeader(e.target.value); setError(""); const row = Number(e.target.value); if (e.target.value && Number.isInteger(row) && row >= 1 && row <= sheet.rows) onChange(sheet.id, { headerRow: row, headerRange: undefined }); }} onBlur={commitHeader} onKeyDown={(e) => { if (e.key === "Enter" && !e.ctrlKey && !e.metaKey) { e.preventDefault(); commitHeader(); e.currentTarget.blur(); } }} /></label>
        <label>来源名称<input aria-label={`${file.name} / ${sheet.name} 来源名称`} title={source?.sourceName} disabled={disabled || !enabled || !source?.enabled} value={source?.sourceName ?? ""} onChange={(e) => onChange(sheet.id, { sourceName: e.target.value })} /></label>
      </div>
      {error && <p className="field-error" id={`header-${sheet.id}`}>{error}</p>}
      {source?.headerRange && <small className="source-config-note">已设置多行表头，修改行号会改为单行表头。</small>}
      {enabled && source?.enabled && !source.headerRange && sheet.suggestedHeaderRow !== source.headerRow && <button className="source-header-suggestion text-button" disabled={disabled} onClick={() => onChange(sheet.id, { headerRow: sheet.suggestedHeaderRow, headerRange: undefined })}>使用识别的第 {sheet.suggestedHeaderRow} 行表头</button>}
    </div>
  </div>;
}
export default function SourceFileList({ files, sources, fileSelections, activeSheet, templateId, disabled = false, onFileSelect, onSourceChange, onPreview, onDelete }: {
  files: SourceFile[]; sources: SourceSelection[]; fileSelections: FileSelection[]; activeSheet: string | null; disabled?: boolean;
  templateId?: string;
  onFileSelect: (fileId: string, enabled: boolean) => void; onSourceChange: (id: string, patch: Partial<SourceSelection>) => void;
  onPreview: (sheetId: string) => void; onDelete: (fileId: string) => void;
}) {
  const [expandedFileId, setExpandedFileId] = useState<string | null>(null);
  const selections = new Map(sources.map((s) => [s.sheetId, s]));
  const activeFile = files.find((f) => f.sheets.some((s) => s.id === activeSheet));
  useEffect(() => { setExpandedFileId(activeFile?.id ?? null); }, [activeFile?.id, activeSheet]);
  return <div className="file-scroll">{files.map((file) => {
    const enabled = fileSelections.find((f) => f.fileId === file.id)?.enabled ?? true;
    const count = file.sheets.filter((s) => selections.get(s.id)?.enabled).length;
    const open = expandedFileId === file.id;
    const info = `${file.name} · ${bytes(file.size)} · 已勾选 ${count} / ${file.sheets.length} 张表`;
    return <div className="source-file" key={file.id}>
      <div className={`source-file-row ${file.id === activeFile?.id ? "active" : ""}`}>
        <label className="source-select" title="启用或暂时排除此文件；工作表选择会保留"><input type="checkbox" checked={enabled} disabled={disabled} aria-label={`启用文件 ${file.name}`} onChange={(e) => onFileSelect(file.id, e.target.checked)} /></label>
        <button className={`source-expand ${open ? "open" : ""}`} aria-label={`${open ? "收起" : "展开"} ${file.name}`} aria-expanded={open} onClick={() => setExpandedFileId(open ? null : file.id)}><ChevronRight size={14} /></button>
        <button className="source-file-preview" title={info} aria-expanded={open} onClick={() => setExpandedFileId(open ? null : file.id)}><span>{file.name}</span></button>
        <small className="source-sheet-count" title={info}>{count}/{file.sheets.length}</small>
        <button className="source-file-delete" title="删除导入副本" disabled={disabled} aria-label={`删除 ${file.name}`} onClick={() => onDelete(file.id)}><Trash2 size={13} /></button>
      </div>
      {open && <div className="source-file-sheets"><small className="source-config-note">{file.id === templateId ? "统一配置：修改后自动用于全部文件的同名 Sheet" : "自动跟随首文件设置，可预览此文件"}</small>{file.sheets.map((sheet) => <SheetRow key={sheet.id} file={file} sheet={sheet} source={selections.get(sheet.id)} enabled={enabled} disabled={disabled || (!!templateId && file.id !== templateId)} active={sheet.id === activeSheet} onChange={onSourceChange} onPreview={onPreview} />)}</div>}
    </div>;
  })}</div>;
}
