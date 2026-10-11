import { expect, it } from "vitest";
import { excelText, formatColour } from "./excelFormat";

it("renders original Excel codes without changing raw values", () => {
  const value={kind:"number",value:1234.5} as const;
  expect(excelText(value,'"¥"#,##0.00')).toBe("¥1,234.50");
  expect(value.value).toBe(1234.5);
  expect(excelText({kind:"number",value:0.125},"0.0%")).toBe("12.5%");
  expect(excelText({kind:"number",value:23},"00000")).toBe("00023");
  expect(excelText({kind:"number",value:-12.5},"0.00;[Red](0.00)")).toBe("(12.50)");
  expect(formatColour("0.00;[Red](0.00)",{kind:"number",value:-12.5})).toBe("red");
  expect(excelText({kind:"text",value:"00123"},"0.00")).toBe("00123");
});
it("keeps original date serials and the 1904 date system", () => {
  expect(excelText({kind:"date",value:"1904-01-01"},"yyyy/mm/dd",0,true)).toBe("1904/01/01");
  expect(excelText({kind:"date",value:"2024-10-02"},'yyyy"年"m"月"d"日"')).toBe("2024年10月2日");
  expect(excelText({kind:"number",value:1.5},"[h]:mm")).toBe("36:00");
});
