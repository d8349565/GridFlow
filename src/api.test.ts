import { expect, test } from "vitest";
import { cellText, coordinate, defaultRules } from "./api";
test("cell coordinate and scalar identity are preserved", () => {
  expect(coordinate(10, 27)).toBe("AA10");
  expect(cellText({ kind: "number", value: 0 })).toBe("0");
  expect(cellText({ kind: "text", value: "00123" })).toBe("00123");
  expect(defaultRules().version).toBe(2);
});
