import { describe, expect, it } from "vitest";
import { defaultRules } from "./api";
import { formatChanges } from "./review";
import type { TypeSuggestion } from "./types";

const hint = (overrides: Partial<TypeSuggestion> = {}): TypeSuggestion => ({
  key: "input:金额", name: "金额", suggestedType: "number",
  examples: [{ before: { kind: "number", value: 0 }, after: { kind: "number", value: 0 }, error: null }],
  ...overrides,
});
describe("user-facing format review", () => {
  it("does not ask to confirm unchanged numbers, zeroes or text", () => {
    expect(formatChanges([hint(), hint({ suggestedType: "text", examples: [{
      before: { kind: "text", value: "00123" }, after: { kind: "text", value: "00123" }, error: null,
    }] })], defaultRules())).toEqual([]);
  });
  it("explains a useful type change even when a date looks identical", () => {
    const date = hint({ suggestedType: "date", examples: [{
      before: { kind: "text", value: "2024-10-02" }, after: { kind: "date", value: "2024-10-02" }, error: null,
    }] });
    expect(formatChanges([date], defaultRules())[0].changes).toHaveLength(1);
  });
  it("keeps failed examples visible and suppresses adjustments already configured", () => {
    const currency = hint({ examples: [
      { before: { kind: "text", value: "¥1,200" }, after: { kind: "number", value: 1200 }, error: null },
      { before: { kind: "text", value: "待确认" }, after: { kind: "text", value: "待确认" }, error: "无法转换为数字" },
    ] });
    const review = formatChanges([currency], defaultRules())[0];
    expect(review.changes).toHaveLength(1);
    expect(review.errors).toHaveLength(1);
    expect(formatChanges([currency], { ...defaultRules(), cleanNumeric: true })).toEqual([]);
    expect(formatChanges([currency], { ...defaultRules(), columns: [{ key: currency.key, name: "销售额", hidden: false, dataType: "number" }] })).toEqual([]);
  });
  it("suppresses hints with zero valid changes and only conversion errors", () => {
    const bogus = hint({
      key: "input:张建国",
      name: "张建国",
      suggestedType: "date",
      examples: [
        { before: { kind: "text", value: "王立群" }, after: { kind: "text", value: "王立群" }, error: "无法确定日期格式" },
      ],
    });
    expect(formatChanges([bogus], defaultRules())).toEqual([]);
  });
});
