import { expect, test } from "vitest";
import { parseLookupTable } from "./renameTable";

test("pasted Excel lookup information preserves codes and quoted cells", () => {
  expect(parseLookupTable('编码\t名称\r\n001\t"客户\t甲"\r\n002\t"第一行\n第二行"\r\n003\t"名称""引用"\r\n').rows).toEqual([
    ["编码", "名称"], ["001", "客户\t甲"], ["002", "第一行\n第二行"], ["003", '名称"引用'],
  ]);
  expect(parseLookupTable('001\t"未闭合').error).not.toBe("");
  expect(parseLookupTable("001\t\n").rows).toEqual([["001", ""]]);
});
