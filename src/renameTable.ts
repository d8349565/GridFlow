import type { RenameLookupTable, RenamePart, RenameRules } from "./types";

export type TableLookup = Extract<RenamePart, { kind: "tableLookup" }>;
export type LookupPaste = { text: string; header: boolean | null };
type TableData = Pick<RenameLookupTable, "columns" | "rows">;

// Excel clipboard TSV may quote embedded tabs, newlines and double quotes.
export function parseLookupTable(text: string): { rows: string[][]; error: string } {
  const rows: string[][] = []; let row: string[] = [], cell = "", quoted = false, closed = false;
  const pushCell = () => { row.push(cell); cell = ""; closed = false; };
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') { cell += '"'; i++; }
      else if (c === '"') { quoted = false; closed = true; }
      else cell += c;
    } else if (c === '"' && !cell && !closed) quoted = true;
    else if (c === "\t") pushCell();
    else if (c === "\r" || c === "\n") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      pushCell(); if (row.some(v => v.trim())) rows.push(row); row = [];
    } else if (closed) return { rows: [], error: "引号后的内容无法识别，请重新从 Excel 复制完整区域。" };
    else cell += c;
  }
  if (quoted) return { rows: [], error: "对照表的引号未闭合，请重新从 Excel 复制完整区域。" };
  pushCell(); if (row.some(v => v.trim())) rows.push(row);
  if (rows.some(v => v.length > 64)) return { rows: [], error: "请只复制需要的对照信息，一次最多 64 列。" };
  if (rows.length > 10001) return { rows: [], error: "对照表最多支持 10000 行数据。" };
  return { rows, error: "" };
}

// Match Rust str::trim in the rename engine.
export const trimLookupText = (text: string) => text.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "");

export function readLookupPaste({ text, header }: LookupPaste, existingColumns?: string[]): { data: TableData | null; error: string } {
  if (!text.trim()) return { data: null, error: "" };
  if (existingColumns && header === null) return { data: null, error: "请先选择粘贴内容是否包含列名。" };
  const parsed = parseLookupTable(text);
  if (parsed.error) return { data: null, error: parsed.error };
  const width = parsed.rows.reduce((max, row) => Math.max(max, row.length), 0);
  if (width < 2) return { data: null, error: "请从 Excel 复制至少两列。" };
  if (!header && existingColumns) {
    const firstRow = new Set(parsed.rows[0].map(trimLookupText));
    if (firstRow.size === existingColumns.length && existingColumns.every(column => firstRow.has(column))) {
      return { data: null, error: "首行与原表的列名一致，请选择「包含列名，按列名对应」。" };
    }
    if (width !== existingColumns.length) {
      return { data: null, error: "只粘贴数据时，列数需要和原表一致。列有变化时，请连同列名复制。" };
    }
  }
  const columns = !header && existingColumns ? existingColumns : Array.from({ length: width }, (_, i) =>
    (header && trimLookupText(parsed.rows[0][i] ?? "")) || `第 ${i + 1} 列`,
  );
  if (new Set(columns).size !== width) return { data: null, error: "列名有重复。请在 Excel 中改为不同的列名后重新粘贴。" };
  const data = header ? parsed.rows.slice(1) : parsed.rows;
  if (!data.length) return { data: null, error: "请同时复制列名下方的数据。" };
  if (data.length > 10000) return { data: null, error: "对照表最多支持 10000 行数据。" };
  return { data: { columns, rows: data.map(row => columns.map((_, i) => row[i] ?? "")) }, error: "" };
}

export function lookupColumnError(table: TableData, column: string): string {
  const index = table.columns.indexOf(column);
  if (index < 0) return `对照表缺少「${column}」列。`;
  const keys = new Set<string>();
  for (const [row, values] of table.rows.entries()) {
    const key = trimLookupText(values[index] ?? "");
    if (!key) return `「${column}」第 ${row + 1} 行为空，请补齐匹配值。`;
    if (keys.has(key)) return `「${column}」列有重复值“${key}”，请保留正确的一行或选择其他匹配列。`;
    keys.add(key);
  }
  return "";
}

export function lookupUpdateError(table: TableData, bindings: TableLookup[]): string {
  const usedColumns = new Set(bindings.flatMap(part => [part.matchColumn, part.valueColumn]));
  const missing = [...usedColumns].find(column => !table.columns.includes(column));
  if (missing !== undefined) return `命名规则正在使用「${missing}」列，请保留该列。`;
  const matchColumns = new Set(bindings.map(part => part.matchColumn));
  for (const column of matchColumns) {
    const error = lookupColumnError(table, column);
    if (error) return error;
  }
  return "";
}

export function availableLookupName(tables: RenameLookupTable[], base = "对照表"): string {
  const names = new Set(tables.map(table => table.name.trim()));
  let name = base;
  for (let n = 2; names.has(name); n++) name = `${base}（${n}）`;
  return name;
}

export function migrateRenameLookups(rules: Omit<RenameRules, "lookupTables"> & { lookupTables?: RenameLookupTable[] }): RenameRules {
  const lookupTables = [...(rules.lookupTables ?? [])];
  const parts = rules.parts.map(part => {
    if (part.kind !== "lookup") return part;
    const valueColumn = trimLookupText(part.name ?? "") || "对照信息";
    const matchColumn = valueColumn === "匹配值" ? "原值" : "匹配值";
    const tableId = crypto.randomUUID();
    lookupTables.push({
      id: tableId,
      name: availableLookupName(lookupTables, `${valueColumn}对照表`),
      columns: [matchColumn, valueColumn],
      rows: part.entries.map(entry => [entry.key, entry.value]),
    });
    return { kind: "tableLookup", source: part.source, tableId, matchColumn, valueColumn, fallback: part.fallback ?? null } satisfies TableLookup;
  });
  return { ...rules, parts, lookupTables };
}
