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
