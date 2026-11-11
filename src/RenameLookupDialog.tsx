import { useMemo, useState } from "react";
import { coordinate } from "./api";
import { Button, Modal } from "./components";
import { LookupPasteFields } from "./RenameLookupTables";
import { availableLookupName, lookupColumnError, readLookupPaste, trimLookupText, type LookupPaste, type TableLookup } from "./renameTable";
import type { RenameCell, RenameLookupTable } from "./types";

export default function RenameLookupDialog({ source, sampleValue, tables, existing, slots, onSave, onClose }: {
  source: RenameCell;
  sampleValue?: string;
  tables: RenameLookupTable[];
  existing?: TableLookup;
  slots: number;
  onSave: (parts: TableLookup[], newTable?: RenameLookupTable) => void;
  onClose: () => void;
}) {
  const [tableId, setTableId] = useState(() => tables.find(table => table.id === existing?.tableId)?.id ?? tables[0]?.id ?? "");
  const [name, setName] = useState(() => availableLookupName(tables));
  const [paste, setPaste] = useState<LookupPaste>({ text: "", header: false });
  const [matchChoice, setMatchChoice] = useState(existing?.matchColumn ?? "");
  const [valueChoices, setValueChoices] = useState<string[] | null>(existing ? [existing.valueColumn] : null);
  const [fallback, setFallback] = useState(existing?.fallback ?? "");
  const parsed = useMemo(() => readLookupPaste(paste), [paste]);
  const table = tables.find(item => item.id === tableId);
  const data = table ?? parsed.data;
  const columns = data?.columns ?? [];
  const matchColumn = columns.includes(matchChoice) ? matchChoice : columns[0] ?? "";
  const defaultValueColumn = columns.find(column => column !== matchColumn);
  const valueColumns = valueChoices?.filter(column => columns.includes(column)) ?? (defaultValueColumn ? [defaultValueColumn] : []);
  const matchError = useMemo(() => data ? lookupColumnError(data, matchColumn) : "", [data, matchColumn]);
  const error = !table && (!name.trim() || tables.some(item => item.name.trim() === name.trim()))
    ? "请为新对照表填写一个未使用的名称。"
    : matchError || (data && !valueColumns.length ? "请选择至少一列加入文件名。" : "")
      || (valueColumns.length > slots ? `文件名最多 64 项，本次还能添加 ${slots} 项。` : "");
  const canSave = !!data && !error && valueColumns.length > 0;
  const sample = sampleValue === undefined ? undefined : trimLookupText(sampleValue);
  const matchIndex = columns.indexOf(matchColumn);
  const matched = canSave && sample ? data.rows.find(row => trimLookupText(row[matchIndex] ?? "") === sample) : undefined;
  const sampleFields = matched ? valueColumns.map(column => {
    const value = trimLookupText(matched[columns.indexOf(column)] ?? "");
    return { column, value };
  }) : [];
  const blankField = sampleFields.find(field => !field.value);
  const hasFallback = !!trimLookupText(fallback);

  function resetColumns() {
    setMatchChoice("");
    setValueChoices(null);
  }

  function save() {
    if (!data || !canSave) return;
    const id = table?.id ?? crypto.randomUUID();
    const newTable = table ? undefined : { ...data, id, name: name.trim() };
    onSave(valueColumns.map(valueColumn => ({ kind: "tableLookup", source, tableId: id, matchColumn, valueColumn, fallback: fallback || null })), newTable);
  }

  return <Modal title={existing ? "修改对照取值" : "从对照表取值"} onClose={onClose} wide>
    <div className="modal-body rename-dialog-body rename-lookup-body">
      <p>用 <strong>{source.sheet}!{coordinate(source.row, source.column)}</strong> 的原值找到对应行，再把选中的列加入文件名。</p>
      {tables.length > 0 && <label className="rename-field">选择对照表<select value={tableId} onChange={e => { setTableId(e.target.value); resetColumns(); }}>
        {tables.map(item => <option value={item.id} key={item.id}>{item.name}</option>)}
        <option value="">新建对照表…</option>
      </select></label>}
      {!table && <>
        <label className="rename-field">对照表名称<input value={name} onChange={e => setName(e.target.value)} /></label>
        <LookupPasteFields paste={paste} parsed={parsed} onChange={next => {
          if (next.header !== paste.header) resetColumns();
          setPaste(next);
        }} />
      </>}
      {data && <div className="rename-lookup-columns">
        <label className="rename-field">按哪一列匹配<select value={matchColumn} onChange={e => setMatchChoice(e.target.value)}>
          {columns.map(column => <option key={column}>{column}</option>)}
        </select></label>
        {existing || columns.length === 2 ? <label className="rename-field">取哪一列加入文件名<select value={valueColumns[0] ?? ""} onChange={e => setValueChoices([e.target.value])}>
          {!valueColumns.length && <option value="" disabled>请选择</option>}
          {columns.map(column => <option key={column}>{column}</option>)}
        </select></label> : <fieldset><legend>取哪些列加入文件名（可多选）</legend>
          <div className="rename-lookup-tools">{columns.map(column => <label className="check-label" key={column}>
            <input type="checkbox" checked={valueColumns.includes(column)} onChange={e => setValueChoices(e.target.checked ? [...valueColumns, column] : valueColumns.filter(value => value !== column))} />{column}
          </label>)}</div>
        </fieldset>}
      </div>}
      {error && <p className="notice error" role="alert">{error}</p>}
      {sample !== undefined && <div className="rename-rule-result rename-lookup-match" role="status">
        <span>当前单元格的值</span>
        <strong>{sample || "空白"}</strong>
        {sampleFields.length > 0 && <p>{sampleFields.map(({ column, value }) => `${column} ${value || "（空白）"}`).join(" · ")}</p>}
        {canSave && (!matched || blankField) && <small>
          {blankField ? `找到了对应行，但「${blankField.column}」为空。` : sample ? "对照表中找不到这个值。" : "当前单元格为空。"}
          {hasFallback ? "将使用备用文字。" : "核对时会跳过这份文件。"}
        </small>}
      </div>}
      <details className="rename-advanced"><summary>找不到对应值时 · {hasFallback ? "使用备用文字" : "跳过文件"}</summary><div className="rename-advanced-body">
        <label className="rename-field">备用文字（可选）<input placeholder="留空则跳过该文件" value={fallback} onChange={e => setFallback(e.target.value)} /></label>
      </div></details>
    </div>
    <div className="modal-footer">
      <Button onClick={onClose}>取消</Button>
      <Button primary disabled={!canSave} onClick={save}>{existing ? "保存取值规则" : "加入文件名"}</Button>
    </div>
  </Modal>;
}
