import { useRef, useState } from "react";
import { Plus, Trash2, MousePointer2 } from "lucide-react";
import { Button } from "./components";
import { call, cellDisplay } from "./api";
import { fileEnabled } from "./summary";
import type { DataPage, FixedCellRule, RuleSet, SourceFile } from "./types";

function addressParts(value: string): { row: number; column: number } | null {
  const match = /^([A-Z]{1,3})([1-9]\d*)$/.exec(value.trim().toUpperCase());
  if (!match) return null;
  const column = [...match[1]].reduce((sum, c) => sum * 26 + c.charCodeAt(0) - 64, 0), row = Number(match[2]);
  return column <= 16_384 && row <= 1_048_576 ? { row, column } : null;
}
export default function FixedInfoColumns({ rules, files, groupNames, onChange, onPick }: {
  rules: RuleSet; files: SourceFile[]; groupNames: string[]; onChange: (rules: FixedCellRule[]) => void; onPick: () => void;
}) {
  const [editing, setEditing] = useState<FixedCellRule | null>(null);
  const [preview, setPreview] = useState("");
  const [busy, setBusy] = useState(false);
  const [specific, setSpecific] = useState(false);
  const revision = useRef(0);
  const names = [...new Set(files.filter((f) => fileEnabled(rules, f.id)).flatMap((f) => f.sheets.map((s) => s.name)))];
  const address = editing ? addressParts(editing.address) : null;
  const overlap = editing && rules.fixedCells.some((r) => r.id !== editing.id && r.name.trim() === editing.name.trim()
    && (!r.targetSheets.length || !editing.targetSheets.length || r.targetSheets.some((n) => editing.targetSheets.includes(n))));
  function patch(value: Partial<FixedCellRule>) { revision.current++; setEditing((old) => old ? { ...old, ...value } : old); setPreview(""); }
  async function previewValue() {
    if (!editing || !address) return;
    setBusy(true); setPreview("");
    const current = ++revision.current;
    try {
      const file = files.find((f) => fileEnabled(rules, f.id));
      const sheetName = names.includes(editing.sheetName) ? editing.sheetName : editing.sheetName.trim();
      const sheet = file?.sheets.find((s) => s.name === sheetName);
      if (!sheet || address.row > sheet.rows || address.column > sheet.width) { setPreview(`${file?.name ?? "首个文件"} 的 ${editing.sheetName}!${editing.address} 缺失，汇总时会留空并提示。`); return; }
      const page = await call<DataPage>("preview_sheet", { sheetId: sheet.id, offset: address.row - 1, limit: 1, headerRow: null });
      if (revision.current === current) setPreview(`${file!.name}：${cellDisplay(page.rows[0]?.[address.column - 1])}`);
    } catch (error) { if (revision.current === current) setPreview((error as { message?: string }).message ?? String(error)); }
    finally { setBusy(false); }
  }
  return <>
    <div className="panel-heading"><h3>从其他 Sheet 补充固定信息</h3><p>例如“基本信息!B2”是部门名称，每份文件各取自己的值，补到该文件的汇总行中。</p></div>
    {rules.fixedCells.map((r) => <div className="setting-card" key={r.id}><div className="setting-row"><button className="text-button" onClick={() => { revision.current++; setEditing({ ...r }); setSpecific(!!r.targetSheets.length); setPreview(""); }}>{r.name} · 修改</button><button aria-label={`删除固定信息列 ${r.name}`} onClick={() => onChange(rules.fixedCells.filter((f) => f.id !== r.id))}><Trash2 size={14} /></button></div><span className="coordinate-badge">{r.sheetName}!{r.address}</span><small>{r.targetSheets.length ? `用于：${r.targetSheets.join("、")}` : "用于所有汇总表"}</small></div>)}
    {editing ? <div className="setting-card fixed-info-editor">
      <label className="stacked-label">信息所在的 Sheet<input list="fixed-info-sheets" value={editing.sheetName} onChange={(e) => patch({ sheetName: e.target.value })} placeholder="例如：基本信息" /><datalist id="fixed-info-sheets">{names.map((name) => <option key={name} value={name} />)}</datalist></label>
      <label className="stacked-label">单元格地址<input value={editing.address} onChange={(e) => patch({ address: e.target.value.toUpperCase() })} placeholder="例如：B2" aria-invalid={!!editing.address && !address} aria-describedby={editing.address && !address ? "fixed-address-error" : undefined} /></label>
      {editing.address && !address && <p id="fixed-address-error" className="field-error">请填写有效的 Excel 单元格地址，例如 B2。</p>}
      <label className="stacked-label">新增列名称<input value={editing.name} onChange={(e) => patch({ name: e.target.value })} placeholder="例如：部门" aria-invalid={!!overlap} aria-describedby={overlap ? "fixed-name-error" : undefined} /></label>
      {overlap && <p id="fixed-name-error" className="field-error">这个应用范围内已有同名信息列，请修改名称。</p>}
      <details className="advanced"><summary>应用范围：{specific ? editing.targetSheets.join("、") || "请选择" : "所有汇总表"}</summary><label className="check-label"><input type="checkbox" checked={!specific} disabled={!groupNames.length} onChange={(e) => { setSpecific(!e.target.checked); patch({ targetSheets: e.target.checked ? [] : [...groupNames] }); }} />所有汇总表</label>{specific && groupNames.map((name) => <label className="check-label" key={name}><input type="checkbox" checked={editing.targetSheets.includes(name)} onChange={(e) => {
        const next = e.target.checked ? [...editing.targetSheets, name] : editing.targetSheets.filter((n) => n !== name);
        patch({ targetSheets: next });
      }} />{name}</label>)}</details>
      <Button disabled={!editing.sheetName.trim() || !address || busy} loading={busy} onClick={() => void previewValue()}>预览取值</Button>
      {preview && <p className="fixed-info-preview" role="status">{preview}</p>}
      {specific && !editing.targetSheets.length && <p className="field-error">请选择至少一张汇总表，或改为用于所有汇总表。</p>}
      <div className="toolbar-group"><Button primary disabled={!editing.sheetName.trim() || !editing.name.trim() || !address || !!overlap || busy || (specific && !editing.targetSheets.length)} onClick={() => {
        const next = { ...editing, name: editing.name.trim(), sheetName: names.includes(editing.sheetName) ? editing.sheetName : editing.sheetName.trim(), address: editing.address.trim().toUpperCase() };
        onChange(rules.fixedCells.some((r) => r.id === next.id) ? rules.fixedCells.map((r) => r.id === next.id ? next : r) : [...rules.fixedCells, next]); setEditing(null);
      }}>保存信息列</Button><Button onClick={() => setEditing(null)}>取消</Button></div>
    </div> : <Button className="full" icon={Plus} onClick={() => { revision.current++; setSpecific(false); setEditing({ id: crypto.randomUUID(), name: "", sheetName: "", address: "", targetSheets: [] }); setPreview(""); }}>新增固定信息列</Button>}
    <Button className="full fixed-info-pick" icon={MousePointer2} onClick={onPick}>也可从原表中点选单元格</Button>
  </>;
}
