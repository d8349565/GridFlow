import { cellText } from "./api";
import type { RuleSet, TypeSuggestion } from "./types";

// A format suggestion is useful only when it would change a sample value.
export function formatChanges(hints: TypeSuggestion[], rules: RuleSet) {
  return hints.flatMap((hint) => {
    const configured = rules.columns.find((c) => c.key === hint.key)?.dataType ?? "auto";
    const effective = configured === "auto"
      ? rules.cleanNumeric && hint.suggestedType === "number" ? "number"
        : rules.parseDates && hint.suggestedType === "date" ? "date" : "auto"
      : configured;
    if (effective === hint.suggestedType || effective === "skip") return [];
    const changes = hint.examples.filter((e) => !e.error && (
      e.before.kind !== e.after.kind || cellText(e.before) !== cellText(e.after)
    ));
    const errors = hint.examples.filter((e) => e.error);
    return changes.length ? [{ ...hint, changes, errors }] : [];
  });
}

export function formatPurpose(type: TypeSuggestion["suggestedType"]) {
  return type === "number" ? "改成数字，可以求和和计算平均值"
    : type === "date" ? "改成日期，可以按时间排序和筛选"
    : type === "text" ? "作为文字保存，保留编号开头的 0"
    : "保留原样";
}
