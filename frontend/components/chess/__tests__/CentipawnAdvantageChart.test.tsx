/**
 * Tests for CentipawnAdvantageChart — FE-57 (#1116)
 *
 * Covers:
 * - Empty-state render
 * - Chart renders with evaluation data
 * - Click-to-jump callback (onPlySelect)
 * - Annotation summary counts
 * - Tooltip content
 */

import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import {
  CentipawnAdvantageChart,
  type EvaluatedPly,
} from "../CentipawnAdvantageChart";

// recharts uses ResizeObserver and SVG APIs not present in jsdom
vi.mock("recharts", async () => {
  const actual = await vi.importActual<typeof import("recharts")>("recharts");
  return {
    ...actual,
    ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
      <div data-testid="responsive-container">{children}</div>
    ),
    AreaChart: ({
      children,
      onClick,
    }: {
      children: React.ReactNode;
      onClick?: (data: unknown) => void;
    }) => (
      <div
        data-testid="area-chart"
        onClick={() =>
          onClick?.({
            activeTooltipIndex: 2, // recharts v3 click data
          })
        }
      >
        {children}
      </div>
    ),
    Area: () => <div data-testid="area" />,
    XAxis: () => null,
    YAxis: () => null,
    CartesianGrid: () => null,
    Tooltip: () => null,
    ReferenceLine: () => null,
  };
});

const PLIES: EvaluatedPly[] = [
  { plyIndex: 0, evaluation: 0, mate: null },
  { plyIndex: 1, evaluation: 30, san: "e4", bestMove: "e4" },
  { plyIndex: 2, evaluation: -50, san: "e5", quality: "inaccuracy" },
  { plyIndex: 3, evaluation: 120, san: "Nf3", quality: "mistake" },
  { plyIndex: 4, evaluation: -300, san: "Nc6", quality: "blunder" },
  { plyIndex: 5, evaluation: 50, san: "Bc4", quality: "brilliant" },
];

describe("CentipawnAdvantageChart (FE-57 / #1116)", () => {
  it("renders the empty-state when no plies are provided", () => {
    render(
      <CentipawnAdvantageChart plies={[]} activePly={0} onPlySelect={vi.fn()} />
    );
    expect(screen.getByText(/no evaluation data/i)).toBeInTheDocument();
  });

  it("renders the chart container when plies are provided", () => {
    render(
      <CentipawnAdvantageChart plies={PLIES} activePly={0} onPlySelect={vi.fn()} />
    );
    expect(screen.getByTestId("centipawn-advantage-chart")).toBeInTheDocument();
    expect(screen.getByTestId("area-chart")).toBeInTheDocument();
  });

  it("calls onPlySelect with the ply index when the chart area is clicked", () => {
    const onPlySelect = vi.fn();
    render(
      <CentipawnAdvantageChart
        plies={PLIES}
        activePly={0}
        onPlySelect={onPlySelect}
      />
    );
    fireEvent.click(screen.getByTestId("area-chart"));
    expect(onPlySelect).toHaveBeenCalledWith(2);
  });

  it("renders annotation summary for blunders, mistakes, inaccuracies, and brilliants", () => {
    render(
      <CentipawnAdvantageChart plies={PLIES} activePly={0} onPlySelect={vi.fn()} />
    );
    // Summary shows counts
    expect(screen.getByText(/1 brilliant/i)).toBeInTheDocument();
    expect(screen.getByText(/1 inaccuracy/i)).toBeInTheDocument();
    expect(screen.getByText(/1 mistake/i)).toBeInTheDocument();
    expect(screen.getByText(/1 blunder/i)).toBeInTheDocument();
  });

  it("renders the legend", () => {
    render(
      <CentipawnAdvantageChart plies={PLIES} activePly={0} onPlySelect={vi.fn()} />
    );
    expect(screen.getByText("Brilliant")).toBeInTheDocument();
    expect(screen.getByText("Inaccuracy")).toBeInTheDocument();
    expect(screen.getByText("Mistake")).toBeInTheDocument();
    expect(screen.getByText("Blunder")).toBeInTheDocument();
  });

  it("renders the Evaluation Chart heading", () => {
    render(
      <CentipawnAdvantageChart plies={PLIES} activePly={0} onPlySelect={vi.fn()} />
    );
    expect(screen.getByText(/evaluation chart/i)).toBeInTheDocument();
  });

  it("does not render annotation summary when there are no annotations", () => {
    const noAnnotations: EvaluatedPly[] = [
      { plyIndex: 0, evaluation: 0 },
      { plyIndex: 1, evaluation: 20, san: "e4" },
    ];
    render(
      <CentipawnAdvantageChart
        plies={noAnnotations}
        activePly={0}
        onPlySelect={vi.fn()}
      />
    );
    // The annotation summary div should not be present
    expect(screen.queryByTestId("annotation-summary")).not.toBeInTheDocument();
  });

  it("accepts a custom className", () => {
    render(
      <CentipawnAdvantageChart
        plies={PLIES}
        activePly={0}
        onPlySelect={vi.fn()}
        className="my-custom-class"
      />
    );
    const container = screen.getByTestId("centipawn-advantage-chart");
    expect(container.className).toContain("my-custom-class");
  });
});
