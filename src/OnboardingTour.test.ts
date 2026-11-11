import { expect, test } from "vitest";
import { TOUR_STEPS } from "./OnboardingTour";

test("Onboarding tour covers the core 4 steps", () => {
  expect(TOUR_STEPS).toHaveLength(4);

  // Step 1: File list
  expect(TOUR_STEPS[0].selector).toBe('[data-tour="file-sidebar"]');
  expect(TOUR_STEPS[0].title).toBe("按文件配置工作表");

  // Step 2: Set header
  expect(TOUR_STEPS[1].selector).toBe('[data-tour="set-header-btn"]');
  expect(TOUR_STEPS[1].title).toBe("确认列名所在行");

  // Step 3: Extract column
  expect(TOUR_STEPS[2].selector).toBe('[data-tour="add-column-btn"]');
  expect(TOUR_STEPS[2].title).toBe("固定信息补充为新列");

  // Step 4: Preview and merge
  expect(TOUR_STEPS[3].selector).toBe('[data-tour="preview-btn"]');
  expect(TOUR_STEPS[3].title).toBe("生成并保存全部汇总表");
});
