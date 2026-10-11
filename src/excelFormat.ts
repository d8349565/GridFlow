import { format } from "ssf";
import { cellText } from "./api";
import type { Cell } from "./types";

export function excelText(cell: Cell | undefined, code = "General", rawNumber?: number | null, date1904 = false): string {
  if (!cell || cell.kind === "empty") return "";
  if (cell.kind === "text") return cell.value;
  if (cell.kind === "bool") return cell.value ? "TRUE" : "FALSE";
  const fmt: string | number = code.startsWith("@builtin:") ? Number(code.slice(9)) : code;
  let number = rawNumber ?? (cell.kind === "number" ? cell.value : null);
  if (number === null && cell.kind === "date") {
    const p = cell.value.match(/^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2}):(\d{2})(\.\d+)?)?/);
    if (p) number = (Date.UTC(+p[1],+p[2]-1,+p[3],+(p[4]??0),+(p[5]??0),+(p[6]??0)) - Date.UTC(1899,11,30))/86400000 + Number(p[7]??0)/86400 - (date1904 ? 1462 : 0);
  }
  if (number === null || !Number.isFinite(number) || code.length>2048) return cellText(cell);
  try { return format(fmt,number,{date1904,dateNF:"yyyy/m/d"}); }
  catch { return cellText(cell); }
}
export function formatColour(code: string, cell: Cell | undefined) {
  if (!cell || cell.kind !== "number") return undefined;
  const sections=code.split(";");const section=sections[cell.value<0 && sections.length>1?1:cell.value===0 && sections.length>2?2:0];
  const colour=section.match(/\[(Black|Blue|Cyan|Green|Magenta|Red|White|Yellow)\]/i)?.[1].toLowerCase();
  return colour;
}
