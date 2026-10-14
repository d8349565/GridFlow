import { useEffect, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { Button } from "./components";
import { call } from "./api";
import type { Column, LookupRule, RuleSet, SourceFile } from "./types";

export default function LookupColumns({ rules, files, columns, onChange }: {
  rules: RuleSet; files: SourceFile[]; columns: Column[]; onChange: (rules: LookupRule[]) => void;
}) {
  const [editing, setEditing] = useState<LookupRule | null>(null);
  const [referenceColumns, setReferenceColumns] = useState<Column[]>([]);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");
  const selectedFiles = files.filter((f) => f.sheets.some((s) => rules.sources.some((r) => r.enabled && r.sheetId === s.id)));
  const sheetNames = [...new Set(selectedFiles.flatMap((f) => f.sheets.map((s) => s.name)))];
  const reference = selectedFiles.flatMap((f) => f.sheets).find((s) => s.name === editing?.sheetName);
  const missingFiles = editing?.sheetName ? selectedFiles.filter((f) => !f.sheets.some((s) => s.name === editing.sheetName)) : [];
  useEffect(() => {
    setReferenceColumns([]); setMessage("");
    if (!reference || !editing || !Number.isInteger(editing.headerRow) || editing.headerRow < 1) { setLoading(false); return; }
    let cancelled = false;
    setLoading(true);
    call<Column[]>("input_columns", { rules: { ...rules, outputSheets: [], sources: [{ sheetId: reference.id, enabled: true, headerRow: editing.headerRow, sourceName: "" }] } })
      .then((value) => { if (!cancelled) setReferenceColumns(value); })
      .catch((error) => { if (!cancelled) setMessage(error.message ?? String(error)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [reference?.id, editing?.headerRow]);

  function patch(value: Partial<LookupRule>) { setEditing((old) => old ? { ...old, ...value } : old); }
  const duplicateName = !!editing && [...columns.filter((c) => c.key !== `lookup:${editing.id}`).map((c) => rules.columns.find((r) => r.key === c.key)?.name ?? c.name)].some((name) => name.trim() === editing.name.trim());
  const complete = editing && editing.name.trim() && editing.sheetName && columns.some((c) => c.key === editing.matchKey) && editing.lookupKey && editing.valueKey
    && Number.isInteger(editing.headerRow) && editing.headerRow >= 1 && !loading && !message && !missingFiles.length && !duplicateName
    && referenceColumns.some((c) => c.key === editing.lookupKey) && referenceColumns.some((c) => c.key === editing.valueKey);
  return <div className="lookup-columns">
    <div className="panel-heading"><h3>按字段匹配补充信息</h3><p>从每份文件的其他 Sheet 匹配信息，例如用员工编号补充部门。仅作为信息来源的 Sheet 无需勾选汇总。</p></div>
    {rules.lookupColumns.map((rule) => <div className="setting-card" key={rule.id}>
      <div className="setting-row"><button className="text-button" onClick={() => setEditing({ ...rule })}>{rule.name} · 修改</button>
        <button aria-label={`删除补充列 ${rule.name}`} onClick={() => onChange(rules.lookupColumns.filter((r) => r.id !== rule.id))}><Trash2 size={14} /></button></div>
      <small>{rule.sheetName} · 第 {rule.headerRow} 行表头 · {rule.duplicateStrategy === "error" ? "重复时停止" : rule.duplicateStrategy === "first" ? "重复取第一条" : "重复取最后一条"}</small>
    </div>)}
    {editing ? <div className="setting-card lookup-editor">
      <label className="stacked-label">新增列名称<input autoFocus aria-invalid={duplicateName || undefined} aria-describedby={duplicateName ? "lookup-name-error" : undefined} value={editing.name} onChange={(e) => patch({ name: e.target.value })} /></label>
      {duplicateName && <p id="lookup-name-error" className="field-error">这个列名已存在，请换一个名称。</p>}
      <label className="stacked-label">补充信息所在的 Sheet<select value={editing.sheetName} onChange={(e) => patch({ sheetName: e.target.value, lookupKey: "", valueKey: "" })}>
        <option value="">选择 Sheet</option>{sheetNames.map((name) => <option key={name}>{name}</option>)}
      </select></label>
      <label className="stacked-label">补充表的表头行<input type="number" min={1} max={reference?.rows ?? 1} value={editing.headerRow || ""} onChange={(e) => patch({ headerRow: Number(e.target.value), lookupKey: "", valueKey: "" })} /></label>
      <label className="stacked-label">汇总数据的关联字段<select value={editing.matchKey} onChange={(e) => patch({ matchKey: e.target.value })}>
        <option value="">选择关联字段</option>{columns.filter((c) => c.key !== "__source" && !c.key.startsWith("fixed:") && !c.key.startsWith("lookup:")).map((c) => <option key={c.key} value={c.key}>{c.name}</option>)}
      </select></label>
      <label className="stacked-label">补充表的关联字段<select disabled={loading || !referenceColumns.length} value={editing.lookupKey} onChange={(e) => patch({ lookupKey: e.target.value })}>
        <option value="">{loading ? "正在读取列名…" : "选择对应字段"}</option>{referenceColumns.map((c) => <option key={c.key} value={c.key}>{c.name}</option>)}
      </select></label>
      <label className="stacked-label">取哪一列作为补充信息<select disabled={loading || !referenceColumns.length} value={editing.valueKey} onChange={(e) => {
        const name = referenceColumns.find((c) => c.key === e.target.value)?.name ?? "";
        patch({ valueKey: e.target.value, name: editing.name || name });
      }}><option value="">选择信息列</option>{referenceColumns.map((c) => <option key={c.key} value={c.key}>{c.name}</option>)}</select></label>
      <details className="advanced"><summary>匹配处理方式</summary>
        <label className="stacked-label">补充表出现重复关联值<select value={editing.duplicateStrategy} onChange={(e) => patch({ duplicateStrategy: e.target.value })}>
          <option value="error">停止并提示（默认）</option><option value="first">取第一条</option><option value="last">取最后一条</option></select></label>
        <label className="stacked-label">找不到对应信息<select value={editing.missingStrategy} onChange={(e) => patch({ missingStrategy: e.target.value })}>
          <option value="blank">留空并记录提示（默认）</option><option value="error">停止并提示</option></select></label>
      </details>
      {missingFiles.length > 0 && <p className="field-error" role="alert">{missingFiles.map((f) => f.name).join("、")} 缺少这张补充表。</p>}
      {message && <p className="field-error" role="alert">{message}</p>}
      <div className="toolbar-group"><Button primary disabled={!complete} onClick={() => {
        if (!complete) return;
        const next = { ...editing, name: editing.name.trim() };
        onChange(rules.lookupColumns.some((r) => r.id === next.id) ? rules.lookupColumns.map((r) => r.id === next.id ? next : r) : [...rules.lookupColumns, next]);
        setEditing(null);
      }}>保存补充列</Button><Button onClick={() => setEditing(null)}>取消</Button></div>
    </div> : <Button className="full" icon={Plus} disabled={!selectedFiles.length} onClick={() => {
      const previous = rules.lookupColumns.at(-1);
      setEditing({ id: crypto.randomUUID(), name: "", sheetName: previous?.sheetName ?? "", headerRow: previous?.headerRow ?? 1,
        matchKey: previous?.matchKey ?? "", lookupKey: previous?.lookupKey ?? "", valueKey: "", duplicateStrategy: previous?.duplicateStrategy ?? "error", missingStrategy: previous?.missingStrategy ?? "blank" });
    }}>添加匹配列</Button>}
  </div>;
}
