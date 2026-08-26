import React from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { ChessVariantSelector } from "@/components/ChessVariantSelector";

describe("ChessVariantSelector", () => {
  it("renders all configured variants and marks the active one", () => {
    render(
      <ChessVariantSelector
        selectedVariant="standard"
        onSelect={() => undefined}
      />,
    );

    expect(
      screen.getByRole("button", { name: /^standard\b/i }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("button", { name: /^rapid\b/i }),
    ).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByText("Blitz")).toBeInTheDocument();
    expect(screen.getByText("Bullet")).toBeInTheDocument();
  });

  it("notifies consumers when the user picks another variant", () => {
    const onSelect = vi.fn();

    render(
      <ChessVariantSelector selectedVariant="standard" onSelect={onSelect} />,
    );

    fireEvent.click(screen.getByRole("button", { name: /^bullet\b/i }));

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith("bullet");
  });
});
