import { expect, test } from "vitest";
import { lookupColumnError, lookupUpdateError, migrateRenameLookups, parseLookupTable, readLookupPaste } from "./renameTable";
import type { RenameRules } from "./types";

test("pasted Excel lookup information preserves codes and quoted cells", () => {
  expect(parseLookupTable('编码\t名称\r\n001\t"客户\t甲"\r\n002\t"第一行\n第二行"\r\n003\t"名称""引用"\r\n').rows).toEqual([
    ["编码", "名称"], ["001", "客户\t甲"], ["002", "第一行\n第二行"], ["003", '名称"引用'],
  ]);
  expect(parseLookupTable('001\t"未闭合').error).not.toBe("");
  expect(parseLookupTable("001\t\n").rows).toEqual([["001", ""]]);
});

test("a table can have different match columns and repeated output values", () => {
  const { data } = readLookupPaste({ text: "名称\t编码\t别名\n示例\t001\tA\n示例\t1\tB", header: true });
  expect(data?.rows).toEqual([["示例", "001", "A"], ["示例", "1", "B"]]);
  expect(lookupColumnError(data!, "编码")).toBe("");
  expect(lookupColumnError(data!, "名称")).toContain("重复值");
  expect(lookupColumnError({ ...data!, rows: [...data!.rows, ["样例", "\u0085001 ", "C"]] }, "编码")).toContain("重复值“001”");
});

test("replacing a table keeps column references and handles missing headers safely", () => {
  const source = { sheet: "说明", row: 1, column: 1 };
  const binding = { kind: "tableLookup" as const, tableId: "items", source, matchColumn: "编号", valueColumn: "规格", fallback: null };
  const { data } = readLookupPaste({ text: "规格\t编号\t类别\n\t001\t样例", header: true });
  expect(lookupUpdateError(data!, [binding])).toBe("");
  expect(lookupUpdateError({ columns: ["编号", "类别"], rows: [["001", "样例"]] }, [binding])).toContain("请保留该列");
  expect(readLookupPaste({ text: "001\t小型", header: false }, ["编号", "规格"]).data?.columns).toEqual(["编号", "规格"]);
  expect(readLookupPaste({ text: "001\t小型", header: false }, ["编号", "规格", "类别"]).error).toContain("列数需要和原表一致");
});

test("replacement cannot silently interpret reordered column headings as data", () => {
  const columns = ["料号", "条码", "品名"];
  const text = "条码\t料号\t品名\n1\t001\t零件甲\n001\t1\t零件乙";
  expect(readLookupPaste({ text, header: null }, columns).error).toContain("是否包含列名");
  expect(readLookupPaste({ text, header: false }, columns).error).toContain("首行与原表的列名一致");
  expect(readLookupPaste({ text, header: true }, columns).data).toEqual({
    columns: ["条码", "料号", "品名"],
    rows: [["1", "001", "零件甲"], ["001", "1", "零件乙"]],
  });
});

test("an existing configuration keeps its old lookup values and fallback", () => {
  const source = { sheet: "说明", row: 1, column: 1 };
  const old: Omit<RenameRules, "lookupTables"> = { parts: [{ kind: "lookup", source, name: "规格", entries: [{ key: "001", value: "小型" }], fallback: "待核" }], checks: [], separator: "_", prefix: "", suffix: "" };
  const rules = migrateRenameLookups(old);
  expect(rules.lookupTables[0].rows).toEqual([["001", "小型"]]);
  expect(rules.parts[0]).toEqual({ kind: "tableLookup", source, tableId: rules.lookupTables[0].id, matchColumn: "匹配值", valueColumn: "规格", fallback: "待核" });
});
