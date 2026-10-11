import { useEffect, useRef } from "react";
import * as echarts from "echarts/core";
import { BarChart, LineChart, PieChart, ScatterChart } from "echarts/charts";
import {
  GridComponent,
  TooltipComponent,
  LegendComponent,
  DataZoomComponent,
  TitleComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { Analysis } from "./types";
import { cellText } from "./api";
echarts.use([
  BarChart,
  LineChart,
  PieChart,
  ScatterChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
  DataZoomComponent,
  TitleComponent,
  CanvasRenderer,
]);
export default function Chart({
  analysis,
  type,
  onFilter,
  onReady,
}: {
  analysis: Analysis;
  type: string;
  onFilter?: (value: string) => void;
  onReady?: (png: () => string) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const chart = echarts.init(ref.current!);
    const dimCount = analysis.spec.rowFields.length;
    const rows = analysis.spec.showRowTotals
      ? analysis.rows.slice(0, -1)
      : analysis.rows;
    const limited = rows.slice(0, type === "scatter" ? 10000 : 1000);
    const categories = limited.map(
      (row) => row.slice(0, dimCount).map(cellText).join(" / ") || "全部",
    );
    const names = analysis.columns.slice(dimCount).map((c) => c.name);
    const palette = ["#2563eb", "#0d9488", "#f59e0b", "#8b5cf6", "#e11d48"];
    const numeric = (row: (typeof rows)[number], i: number) =>
      row[i]?.kind === "number"
        ? Number((row[i] as { value: number }).value)
        : null;
    const series =
      type === "pie"
        ? [
            {
              type: "pie",
              radius: ["34%", "68%"],
              center: ["50%", "48%"],
              data: limited.map((row, i) => ({
                name: categories[i],
                value: numeric(row, dimCount),
              })),
              label: { formatter: "{b}: {d}%" },
            },
          ]
        : names.map((name, j) => ({
            name,
            type,
            data:
              type === "scatter"
                ? limited.map((row, i) => [
                    row[0]?.kind === "number" ? numeric(row, 0) : i,
                    numeric(row, dimCount + j),
                  ])
                : limited.map((row) => numeric(row, dimCount + j)),
            smooth: type === "line",
            symbolSize: type === "scatter" ? 8 : 5,
            barMaxWidth: 32,
          }));
    chart.setOption({
      color: palette,
      animation: !window.matchMedia("(prefers-reduced-motion: reduce)").matches,
      animationDuration: 180,
      tooltip: {
        renderMode: "richText",
        trigger: type === "pie" || type === "scatter" ? "item" : "axis",
      },
      legend: { bottom: 10, type: "scroll" },
      grid: { top: 35, right: 25, left: 65, bottom: 75, containLabel: true },
      xAxis:
        type === "pie"
          ? undefined
          : type === "scatter"
            ? { type: "value", name: analysis.columns[0]?.name }
            : {
                type: "category",
                data: categories,
                axisLabel: { hideOverlap: true },
              },
      yAxis: type === "pie" ? undefined : { type: "value" },
      dataZoom:
        type === "pie"
          ? undefined
          : [{ type: "inside" }, { type: "slider", bottom: 40, height: 16 }],
      series,
    });
    chart.on("click", (event) => {
      if (dimCount === 1 && typeof event.name === "string")
        onFilter?.(event.name);
    });
    onReady?.(() =>
      chart.getDataURL({
        type: "png",
        pixelRatio: 2,
        backgroundColor: "#ffffff",
      }),
    );
    const observer = new ResizeObserver(() => chart.resize());
    observer.observe(ref.current!);
    return () => {
      observer.disconnect();
      chart.dispose();
    };
  }, [analysis, type]);
  const points = analysis.spec.showRowTotals
    ? analysis.rows.length - 1
    : analysis.rows.length;
  return (
    <div className="chart-wrap">
      {points > (type === "scatter" ? 10000 : 1000) && (
        <div className="notice">
          图表显示前 {type === "scatter" ? "10,000" : "1,000"}{" "}
          组；完整数据仍可导出，请筛选后查看全部类别。
        </div>
      )}
      <div ref={ref} className="chart-canvas" />
    </div>
  );
}
