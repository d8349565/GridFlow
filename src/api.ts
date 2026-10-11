import { invoke, isTauri } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { AppError, Cell, RuleSet, DatasetQuery } from "./types";

export async function call<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (typeof error === "object" && error !== null && "message" in error)
      throw error;
    throw {
      code: "REQUEST",
      message: String(error),
      field: null,
    } satisfies AppError;
  }
}
export { isTauri };
export async function chooseFiles(
  directory = false,
  configuration = false,
): Promise<string[]> {
  const result = await open({
    multiple: !directory,
    directory,
    filters: directory
      ? undefined
      : [
          {
            name: configuration ? "汇总配置" : "Excel / CSV",
            extensions: configuration ? ["json"] : ["xlsx", "xls", "csv"],
          },
        ],
  });
  return result ? (Array.isArray(result) ? result : [result]) : [];
}
export async function savePath(
  name: string,
  format: string,
  directory?: string | null,
): Promise<string | null> {
  const path = directory ? `${directory.replace(/[\\/]$/, "")}\\${name}` : name;
  return save({
    defaultPath: path,
    filters: [{ name: format.toUpperCase(), extensions: [format] }],
  });
}
export function cellText(cell: Cell | undefined): string {
  return !cell || cell.kind === "empty" ? "" : String(cell.value);
}
export function cellDisplay(cell: Cell | undefined): string {
  if (!cell || cell.kind === "empty") return "—";
  if (cell.kind === "number")
    return new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 8 }).format(
      cell.value,
    );
  if (cell.kind === "bool") return cell.value ? "是" : "否";
  return cell.value;
}
export function bytes(value: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  while (value >= 1024 && i < 3) {
    value /= 1024;
    i++;
  }
  return `${value.toFixed(i ? 1 : 0)} ${units[i]}`;
}
export function queryFor(id: string): DatasetQuery {
  return {
    datasetId: id,
    offset: 0,
    limit: 100,
    sorts: [],
    filters: [],
    search: "",
  };
}
export function defaultRules(): RuleSet {
  return {
    version: 2,
    sources: [],
    columns: [],
    mappings: [],
    fixedCells: [],
    mergeStrategy: "outer",
    removeEmptyRows: false,
    removeDuplicates: false,
    duplicateColumns: [],
    keepStrategy: "first",
    cleanNumeric: false,
    parseDates: false,
    includeSource: true,
    dateFormats: {},
    emptyColumns: [],
  };
}
export function coordinate(row: number, column: number): string {
  let letters = "";
  while (column > 0) {
    column--;
    letters = String.fromCharCode(65 + (column % 26)) + letters;
    column = Math.floor(column / 26);
  }
  return `${letters}${row}`;
}
