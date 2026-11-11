import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { Button, Modal } from "./components";
import { availableLookupName, lookupUpdateError, readLookupPaste, type LookupPaste, type TableLookup } from "./renameTable";
import type { RenameLookupTable } from "./types";

export function LookupPasteFields({ paste, onChange, parsed, replacing = false }: {
  paste: LookupPaste;
  onChange: (paste: LookupPaste) => void;
  parsed: ReturnType<typeof readLookupPaste>;
  replacing?: boolean;
}) {
  const input = useRef<HTMLTextAreaElement>(null);
  const errorId = useId();
  useEffect(() => { input.current?.focus(); }, []);
  const { data, error } = parsed;

  return <>
    <label className="rename-field">从 Excel 粘贴对照数据
      <textarea ref={input} rows={3} spellCheck={false} value={paste.text} onChange={e => onChange({ ...paste, text: e.target.value })} aria-invalid={!!error || undefined} aria-describedby={error ? errorId : undefined} />
    </label>
    <small>{replacing ? "连同列名复制，调换列顺序后也能正确对应。" : "复制用于匹配和需要取值的列。可以一次粘贴多列。"}</small>
    {replacing ? <label className="rename-field">粘贴格式
      <select value={paste.header === null ? "" : paste.header ? "headers" : "data"} onChange={e => onChange({ ...paste, header: e.target.value === "headers" })}>
        <option value="" disabled>请选择是否包含列名</option>
        <option value="headers">包含列名，按列名对应</option>
        <option value="data">只有数据，沿用原表列顺序</option>
      </select>
    </label> : <label className="check-label"><input type="checkbox" checked={paste.header === true} onChange={e => onChange({ ...paste, header: e.target.checked })} />首行是列名</label>}
    {error && <p id={errorId} className="notice error" role="alert">{error}</p>}
    {data && <details className="rename-advanced" open={replacing}>
      <summary>预览对照表 · {data.rows.length} 行 / {data.columns.length} 列</summary>
      <div className="rename-table-scroll rename-lookup-grid" tabIndex={0}>
        <table aria-label="粘贴的对照表">
          <thead><tr>{data.columns.map(column => <th scope="col" key={column}>{column}</th>)}</tr></thead>
          <tbody>{data.rows.slice(0, 3).map((row, i) => <tr key={i}>{row.map((value, c) => <td key={c}>{value}</td>)}</tr>)}</tbody>
        </table>
      </div>
    </details>}
  </>;
}

function LookupRowsEditor({ columns, rows, onChange, onPaste }: {
  columns: string[];
  rows: string[][];
  onChange: (rows: string[][]) => void;
  onPaste: () => void;
}) {
  const pageSize = 8;
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(0);
  const [focusedRow, setFocusedRow] = useState<number | null>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  useEffect(() => { searchInput.current?.focus(); }, []);
  const query = search.trim().toLowerCase();
  const indices = rows.flatMap((row, i) =>
    i === focusedRow || !query || row.some(value => value.toLowerCase().includes(query)) ? [i] : [],
  );
  const pages = Math.ceil(indices.length / pageSize);
  const current = Math.min(page, Math.max(0, pages - 1));

  return <>
    <div className="rename-lookup-tools">
      <input ref={searchInput} type="search" aria-label="搜索对照表" placeholder="搜索任一列的内容" value={search} onChange={e => { setSearch(e.target.value); setPage(0); }} />
      <Button icon={Plus} disabled={rows.length >= 10000} onClick={() => {
        setSearch(""); setPage(Math.floor(rows.length / pageSize)); setFocusedRow(rows.length);
        onChange([...rows, columns.map(() => "")]);
      }}>新增一行</Button>
      <Button onClick={onPaste}>粘贴替换整表</Button>
    </div>
    <div className="rename-lookup-tools">
      <small>{query ? `找到 ${indices.length} 行，共 ${rows.length} 行` : `共 ${rows.length} 行`}</small>
      {pages > 1 && <div className="rename-pagination"><Button disabled={!current} onClick={() => setPage(current - 1)}>上一页</Button><span>{current + 1} / {pages}</span><Button disabled={current + 1 >= pages} onClick={() => setPage(current + 1)}>下一页</Button></div>}
    </div>
    {indices.length ? <div className="rename-table-scroll rename-lookup-grid rename-lookup-edit">
      <table aria-label="对照表数据" style={{ minWidth: columns.length * 120 + 66 }}>
        <thead><tr>{columns.map(column => <th scope="col" key={column}>{column}</th>)}<th scope="col">操作</th></tr></thead>
        <tbody>{indices.slice(current * pageSize, (current + 1) * pageSize).map(index =>
          <tr key={index} onFocus={() => setFocusedRow(index)} onBlur={e => {
            if (!e.currentTarget.contains(e.relatedTarget)) setFocusedRow(null);
          }}>
            {columns.map((column, c) => <td key={column}><textarea rows={1} autoFocus={c === 0 && focusedRow === index} aria-label={`第 ${index + 1} 行 ${column}`} spellCheck={false} value={rows[index][c] ?? ""} onChange={e => {
              const next = [...rows];
              next[index] = columns.map((_, i) => i === c ? e.target.value : rows[index][i] ?? "");
              onChange(next);
            }} /></td>)}
            <td><Button icon={Trash2} className="quiet icon-button" title="删除此行" aria-label={`删除第 ${index + 1} 行`} onClick={() => {
              setFocusedRow(null); setPage(current); onChange(rows.filter((_, i) => i !== index));
            }} /></td>
          </tr>,
        )}</tbody>
      </table>
    </div> : <p className="rename-empty-guide">{query ? "没有找到符合搜索条件的记录。" : "对照表为空，可新增一行或从 Excel 粘贴。"}</p>}
  </>;
}

function LookupTableEditor({ table, tables, bindings, onSave, onClose }: {
  table?: RenameLookupTable;
  tables: RenameLookupTable[];
  bindings: TableLookup[];
  onSave: (table: RenameLookupTable) => void;
  onClose: () => void;
}) {
  const [name, setName] = useState(() => table?.name ?? availableLookupName(tables));
  const [rows, setRows] = useState(table?.rows ?? []);
  const [pasting, setPasting] = useState(!table);
  const [paste, setPaste] = useState<LookupPaste>({ text: "", header: false });
  const parsed = useMemo(() => readLookupPaste(paste, table?.columns), [paste, table?.columns]);
  const data = pasting ? parsed.data : table && { columns: table.columns, rows };
  const usedBy = table ? bindings.filter(binding => binding.tableId === table.id) : [];
  const error = !name.trim() ? "请填写对照表名称。"
    : tables.some(other => other.id !== table?.id && other.name.trim() === name.trim()) ? "已有同名对照表，请换一个名称。"
    : data && (!data.rows.length ? "请至少保留一行数据。" : lookupUpdateError(data, usedBy));
  return <Modal title={table ? "编辑对照表" : "新建对照表"} onClose={onClose} wide>
    <div className="modal-body rename-dialog-body rename-lookup-body">
      <label className="rename-field">对照表名称<input value={name} onChange={e => setName(e.target.value)} /></label>
      {usedBy.length > 0 && <small>保存后会更新使用此表的 {usedBy.length} 个命名项。</small>}
      {pasting ? <>
        {table && <div><Button className="quiet" onClick={() => setPasting(false)}>返回表格编辑</Button></div>}
        <LookupPasteFields paste={paste} onChange={setPaste} parsed={parsed} replacing={!!table} />
      </> : table && <LookupRowsEditor columns={table.columns} rows={rows} onChange={setRows} onPaste={() => {
        setPaste({ text: "", header: null }); setPasting(true);
      }} />}
      {error && <p className="notice error" role="alert">{error}</p>}
    </div>
    <div className="modal-footer">
      <Button onClick={onClose}>取消</Button>
      <Button primary disabled={!data || !!error} onClick={() => {
        if (data && !error) onSave({ ...data, id: table?.id ?? crypto.randomUUID(), name: name.trim() });
      }}>{table && pasting ? "保存并替换对照表" : "保存对照表"}</Button>
    </div>
  </Modal>;
}

export default function RenameLookupTables({ tables, bindings, onChange, onClose }: {
  tables: RenameLookupTable[];
  bindings: TableLookup[];
  onChange: (tables: RenameLookupTable[]) => void;
  onClose: () => void;
}) {
  const [editing, setEditing] = useState<RenameLookupTable | "new" | null>(null);
  const [removing, setRemoving] = useState<RenameLookupTable | null>(null);
  if (editing) return <LookupTableEditor table={editing === "new" ? undefined : editing} tables={tables} bindings={bindings} onClose={() => setEditing(null)} onSave={table => {
    onChange(editing === "new" ? [...tables, table] : tables.map(old => old.id === table.id ? table : old));
    setEditing(null);
  }} />;
  return <Modal title="管理对照表" onClose={onClose} wide>
    <div className="modal-body rename-dialog-body rename-lookup-body">
      <p>对照表保存在当前命名配置中，可以供多个命名项使用。</p>
      {removing ? <p role="alert">删除「{removing.name}」？</p> : tables.length ? <ul className="rename-lookup-list">
        {tables.map(table => {
          const used = bindings.filter(binding => binding.tableId === table.id).length;
          return <li key={table.id}>
            <div><strong>{table.name}</strong><small>{table.rows.length} 行 · {table.columns.length} 列 · {used ? `${used} 个命名项使用` : "尚未使用"}</small></div>
            <Button onClick={() => setEditing(table)}>编辑</Button>
            <Button icon={Trash2} className="quiet icon-button" aria-label={`删除对照表 ${table.name}`} title={used ? "请先修改或移除引用此表的命名项" : "删除对照表"} disabled={!!used} onClick={() => setRemoving(table)} />
          </li>;
        })}
      </ul> : <p className="rename-empty-guide">还没有对照表。可以从 Excel 粘贴一张表，之后在命名时直接选择它。</p>}
    </div>
    <div className="modal-footer">{removing ? <>
      <Button onClick={() => setRemoving(null)}>取消</Button>
      <Button danger onClick={() => { onChange(tables.filter(table => table.id !== removing.id)); setRemoving(null); }}>确认删除</Button>
    </> : <>
      <Button onClick={onClose}>关闭</Button>
      <Button primary icon={Plus} onClick={() => setEditing("new")}>新建对照表</Button>
    </>}</div>
  </Modal>;
}
