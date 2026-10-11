import { useEffect, useLayoutEffect, useState, useRef } from "react";
import { X, ArrowRight, ArrowLeft, Check, Sparkles } from "lucide-react";
import { Button } from "./components";

export interface TourStep {
  selector: string;
  title: string;
  description: string;
  placement: "bottom" | "right" | "bottom-left" | "center";
}

export const TOUR_STEPS: TourStep[] = [
  {
    selector: '[data-tour="file-sidebar"]',
    title: "多表集中管理",
    description: "左侧列出了所有待汇总的表格。已自动识别各工作表，点击任意表格可在右侧高保真预览原貌；日常也可随时点击「添加文件」导入更多表格。",
    placement: "right",
  },
  {
    selector: '[data-tour="set-header-btn"]',
    title: "框选并设定表头",
    description: "在中间表格预览区中，按住鼠标左键划选包含列名的整行（例如示例中的第 2 行），然后点击「设为表头」。系统会自动抽取列名并多表智能对齐。",
    placement: "bottom",
  },
  {
    selector: '[data-tour="add-column-btn"]',
    title: "孤立信息一键平铺",
    description: "若报表顶部有独立的“门店名称”或“统计周期”单据属性（如示例中的 A1 格），单选该格并点击「添加为新列」，即可自动转为整张母表的新维度！",
    placement: "bottom",
  },
  {
    selector: '[data-tour="preview-btn"]',
    title: "毫秒级合并与导出",
    description: "配置完成后，点击右上角醒目的「预览结果」（或快捷键 Ctrl+Enter）。引擎将在本地毫秒级生成规范对齐的母表，随后即可自由筛选、分类汇总并导出！",
    placement: "bottom-left",
  },
];

interface OnboardingTourProps {
  active: boolean;
  step: number;
  onStepChange: (step: number) => void;
  onClose: () => void;
  onComplete: () => void;
}

export default function OnboardingTour({
  active,
  step,
  onStepChange,
  onClose,
  onComplete,
}: OnboardingTourProps) {
  const [rect, setRect] = useState<DOMRect | null>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const currentStep = TOUR_STEPS[step] ?? TOUR_STEPS[0];
  const isLast = step === TOUR_STEPS.length - 1;

  const updatePosition = () => {
    if (!active) return;
    const el = document.querySelector(currentStep.selector);
    if (el) {
      setRect(el.getBoundingClientRect());
    } else {
      setRect(null);
    }
  };

  useLayoutEffect(() => {
    updatePosition();
    const timer = setTimeout(updatePosition, 100);
    return () => clearTimeout(timer);
  }, [active, step, currentStep.selector]);

  useEffect(() => {
    if (!active) return;
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [active, currentStep.selector]);

  useEffect(() => {
    if (!active) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowRight" || e.key === "Enter") {
        e.preventDefault();
        if (isLast) onComplete();
        else onStepChange(step + 1);
      } else if (e.key === "ArrowLeft" && step > 0) {
        e.preventDefault();
        onStepChange(step - 1);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [active, step, isLast, onStepChange, onClose, onComplete]);

  if (!active) return null;

  // 计算聚光灯样式
  const spotlightStyle: React.CSSProperties = rect
    ? {
        top: Math.max(0, rect.top - 4),
        left: Math.max(0, rect.left - 4),
        width: rect.width + 8,
        height: rect.height + 8,
      }
    : {
        top: "50%",
        left: "50%",
        width: 0,
        height: 0,
      };

  // 计算弹出气泡位置
  const popoverStyle: React.CSSProperties = {
    position: "fixed",
    zIndex: 9999,
  };

  const popoverWidth = 340;
  if (rect) {
    let top = 0;
    let left = 0;

    if (currentStep.placement === "right") {
      left = rect.right + 14;
      top = rect.top + rect.height / 2 - 80;
    } else if (currentStep.placement === "bottom-left") {
      left = rect.right - popoverWidth;
      top = rect.bottom + 14;
    } else {
      // 默认 bottom
      left = rect.left + rect.width / 2 - popoverWidth / 2;
      top = rect.bottom + 14;
    }

    // 边界防护，防止超出屏幕
    left = Math.max(16, Math.min(window.innerWidth - popoverWidth - 16, left));
    top = Math.max(16, Math.min(window.innerHeight - 260, top));

    popoverStyle.top = `${top}px`;
    popoverStyle.left = `${left}px`;
  } else {
    popoverStyle.top = "50%";
    popoverStyle.left = "50%";
    popoverStyle.transform = "translate(-50%, -50%)";
  }

  return (
    <div className="tour-overlay" onClick={onClose} role="dialog" aria-modal="true" aria-label="新手操作指引">
      {/* 聚光灯镂空框 */}
      <div
        className="tour-spotlight-box"
        style={spotlightStyle}
        onClick={(e) => e.stopPropagation()}
      />

      {/* 气泡卡片 */}
      <div
        ref={popoverRef}
        className="tour-popover"
        style={popoverStyle}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="tour-popover-header">
          <div className="tour-step-badge">
            <Sparkles size={13} className="tour-badge-icon" />
            <span>核心指引 {step + 1} / {TOUR_STEPS.length}</span>
          </div>
          <button
            type="button"
            className="tour-close-btn"
            aria-label="关闭指引"
            onClick={onClose}
          >
            <X size={15} />
          </button>
        </div>

        <div className="tour-popover-body">
          <h3 className="tour-title">{currentStep.title}</h3>
          <p className="tour-desc">{currentStep.description}</p>
        </div>

        <div className="tour-popover-footer">
          <button type="button" className="tour-skip-btn" onClick={onClose}>
            跳过
          </button>

          <div className="tour-dots">
            {TOUR_STEPS.map((_, i) => (
              <span
                key={i}
                className={`tour-dot ${i === step ? "active" : ""}`}
                onClick={() => onStepChange(i)}
                title={`跳转到第 ${i + 1} 步`}
              />
            ))}
          </div>

          <div className="tour-buttons">
            {step > 0 && (
              <Button
                className="quiet"
                icon={ArrowLeft}
                onClick={() => onStepChange(step - 1)}
              >
                上一步
              </Button>
            )}
            <Button
              primary
              icon={isLast ? Check : ArrowRight}
              onClick={() => (isLast ? onComplete() : onStepChange(step + 1))}
            >
              {isLast ? "完成指引" : "下一步"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
