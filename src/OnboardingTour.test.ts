import { expect, test } from "vitest";
import { TOUR_STEPS } from "./OnboardingTour";

test("Onboarding tour covers the core 4 steps", () => {
  expect(TOUR_STEPS).toHaveLength(4);

  // Step 1: File list
  expect(TOUR_STEPS[0].selector).toBe('[data-tour="file-sidebar"]');
  expect(TOUR_STEPS[0].title).toContain("多表集中管理");

  // Step 2: Set header
  expect(TOUR_STEPS[1].selector).toBe('[data-tour="set-header-btn"]');
  expect(TOUR_STEPS[1].title).toContain("设定表头");

  // Step 3: Extract column
  expect(TOUR_STEPS[2].selector).toBe('[data-tour="add-column-btn"]');
  expect(TOUR_STEPS[2].title).toContain("孤立信息");

  // Step 4: Preview and merge
  expect(TOUR_STEPS[3].selector).toBe('[data-tour="preview-btn"]');
  expect(TOUR_STEPS[3].title).toContain("合并与导出");
});
