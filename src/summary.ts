import type { ColumnRule, OutputSheet, RuleSet, SourceFile, SourceSelection } from "./types";

export function mergeMode(rules: RuleSet): string {
  return rules.mergeMode ?? (rules.outputSheets.length ? "legacy" : "single");
}
export function fileEnabled(rules: RuleSet, fileId: string): boolean {
  return rules.fileSelections.find((f) => f.fileId === fileId)?.enabled ?? true;
}
export function executionRules(root: RuleSet, files: SourceFile[], output?: OutputSheet): RuleSet {
  if (mergeMode(root) === "legacy") return output?.rules ?? root;
  const sheets = new Map(files.flatMap((f) => f.sheets.map((s) => [s.id, s] as const)));
  const group = output?.sourceSheetName;
  const sources = root.sources.map((s) => ({ ...s, enabled: s.enabled && !!sheets.get(s.sheetId)
    && fileEnabled(root, sheets.get(s.sheetId)!.fileId) && (!group || sheets.get(s.sheetId)!.name === group) }));
  return {
    ...(output?.rules ?? root), mergeMode: "single", fileSelections: [], outputSheets: [],
    sources,
    fixedCells: root.fixedCells.filter((r) => !r.targetSheets.length || (group ? r.targetSheets.includes(group) : sources.some((s) => s.enabled && r.targetSheets.includes(sheets.get(s.sheetId)!.name)))),
  };
}
export function activeOutputs(root: RuleSet, files: SourceFile[]): OutputSheet[] {
  if (mergeMode(root) === "single") return [];
  return mergeMode(root) === "legacy" ? root.outputSheets : root.outputSheets.filter((o) => executionRules(root, files, o).sources.some((s) => s.enabled));
}
function uniqueOutputName(value: string, used: Set<string>): string {
  let base = value.replace(/[\[\]:*?/\\\x00-\x1f]/g, "_").trim().replace(/^'+|'+$/g, "");
  if (!base || base.toLowerCase() === "history") base = "汇总数据";
  let number = 1;
  for (;;) {
    const suffix = number === 1 ? "" : ` (${number})`;
    const name = [...base].slice(0, 31 - suffix.length).join("") + suffix;
    if (!used.has(name.toLowerCase())) { used.add(name.toLowerCase()); return name; }
    number++;
  }
}
export function syncGroups(root: RuleSet, files: SourceFile[]): RuleSet {
  root = syncFirstFile(root, files);
  if (mergeMode(root) !== "bySheet") return root;
  const names = new Map(files.flatMap((f) => f.sheets.map((s) => [s.id, s.name] as const)));
  const outputs = [...root.outputSheets];
  const used = new Set(outputs.map((o) => o.name.trim().toLowerCase()));
  for (const source of root.sources.filter((s) => s.enabled)) {
    const name = names.get(source.sheetId);
    if (!name || outputs.some((o) => o.sourceSheetName === name)) continue;
    outputs.push({ id: crypto.randomUUID(), name: uniqueOutputName(name, used), sourceSheetName: name, sheets: [],
      rules: { ...structuredClone(root), mergeMode: "single", fileSelections: [], sources: [], outputSheets: [], fixedCells: [] } });
  }
  return { ...root, outputSheets: outputs };
}
export function editConfiguration(root: RuleSet, files: SourceFile[], output: OutputSheet | undefined, change: (rules: RuleSet) => RuleSet): RuleSet {
  const before = executionRules(root, files, output);
  const next = change(structuredClone(before));
  if (mergeMode(root) === "legacy" && output) return { ...root, outputSheets: root.outputSheets.map((o) => o.id === output.id ? { ...o, rules: next } : o) };
  const sources = root.sources.map((source) => {
    const previous = before.sources.find((s) => s.sheetId === source.sheetId);
    const after = next.sources.find((s) => s.sheetId === source.sheetId);
    if (!previous || !after) return source;
    return { ...after, enabled: previous.enabled === after.enabled ? source.enabled : after.enabled };
  });
  if (output) return syncGroups({ ...root, sources, outputSheets: root.outputSheets.map((o) => o.id !== output.id ? o : { ...o,
    rules: { ...next, sources: [], fixedCells: [], fileSelections: [], outputSheets: [], mergeMode: "single" } }) }, files);
  return syncGroups({ ...next, sources, mergeMode: root.mergeMode, outputSheets: root.outputSheets, fileSelections: root.fileSelections, fixedCells: root.fixedCells }, files);
}
export function syncOutputSources(root: RuleSet, sources: SourceSelection[], files: SourceFile[], added: Set<string>, append: boolean): RuleSet {
  const old = new Map(root.sources.map((s) => [s.sheetId, s]));
  const addedFiles = new Set(files.filter((f) => f.sheets.some((s) => added.has(s.id))).map((f) => f.id));
  return syncGroups({ ...root, sources: sources.map((s) => old.get(s.sheetId) ?? s), fileSelections: files.map((f) => ({
    fileId: f.id, enabled: addedFiles.has(f.id) || (append && fileEnabled(root, f.id)),
  })) }, files);
}
export function templateFile(root: RuleSet, files: SourceFile[]): SourceFile | undefined {
  return files.find((f) => fileEnabled(root, f.id));
}
export function syncFirstFile(root: RuleSet, files: SourceFile[]): RuleSet {
  if (mergeMode(root) === "legacy") return root;
  const first = templateFile(root, files);
  if (!first) return root;
  const settings = new Map(first.sheets.map((s) => [s.name, root.sources.find((r) => r.sheetId === s.id)]));
  const sheets = new Map(files.flatMap((f) => f.sheets.map((s) => [s.id, { sheet: s, file: f }] as const)));
  return { ...root, sources: root.sources.map((source) => {
    const entry = sheets.get(source.sheetId);
    if (!entry || entry.file.id === first.id) return source;
    const template = settings.get(entry.sheet.name);
    if (!template) return { ...source, enabled: false };
    const firstStem = first.name.replace(/\.(xlsx?|csv)$/i, "");
    const sourceName = template.sourceName.includes(first.name) ? template.sourceName.split(first.name).join(entry.file.name)
      : firstStem ? template.sourceName.split(firstStem).join(entry.file.name.replace(/\.(xlsx?|csv)$/i, "")) : template.sourceName;
    return { ...source, enabled: template.enabled, headerRow: template.headerRow, headerRange: template.headerRange, sourceName };
  }) };
}
export function templateProblems(root: RuleSet, files: SourceFile[]): string[] {
  if (mergeMode(root) === "legacy") return [];
  const first = templateFile(root, files);
  if (!first) return [];
  const selected = first.sheets.flatMap((sheet) => {
    const source = root.sources.find((s) => s.sheetId === sheet.id && s.enabled);
    return source ? [{ sheet, source }] : [];
  });
  return files.filter((f) => fileEnabled(root, f.id)).flatMap((file) => selected.flatMap(({ sheet, source }) => {
    const target = file.sheets.find((s) => s.name === sheet.name);
    if (!target) return [`${file.name} 缺少已选工作表「${sheet.name}」，请补齐或暂时排除此文件。`];
    if (source.headerRow < 1 || source.headerRow > target.rows || (source.headerRange && source.headerRange.endColumn > target.width))
      return [`${file.name} / ${sheet.name} 无法使用首文件的表头范围，请检查原文件。`];
    return [];
  }));
}
export function outputNameError(value: string, otherNames: string[] = []): string {
  const name = value.trim();
  if (!name || [...name].length > 31 || /[\[\]:*?/\\\x00-\x1f]/.test(name) || name.startsWith("'") || name.endsWith("'")) return "表名需为 1～31 个字符，不能包含 []:*?/\\ 或以单引号开头、结尾。";
  if (otherNames.some((n) => n.trim().toLowerCase() === name.toLowerCase())) return `结果表名称「${name}」重复，请修改。`;
  return name.toLowerCase() === "history" ? "History 是 Excel 保留表名，请换一个名称。" : "";
}
export function removeOutputColumn(rules: RuleSet, columns: ColumnRule[], key: string): RuleSet {
  const sameKey = (value: string) => value === key || (!value.startsWith("fixed:") && !value.startsWith("lookup:") && value !== "__source" && `input:${value}` === key);
  return { ...rules, columns: columns.map((c) => c.key === key ? { ...c, hidden: false, dataType: "skip" } : c),
    duplicateColumns: rules.duplicateColumns.filter((k) => !sameKey(k)), emptyColumns: rules.emptyColumns.filter((k) => !sameKey(k)) };
}
export function outputError(root: RuleSet, files: SourceFile[]): string {
  const problem = templateProblems(root, files)[0];
  if (problem) return problem;
  const outputs = activeOutputs(root, files);
  const names: string[] = [];
  for (const output of outputs) {
    const error = outputNameError(output.name, names);
    if (error) return error;
    names.push(output.name);
  }
  return outputs.length > 32 ? "一次最多汇总 32 张结果表，请分批处理。" : "";
}
