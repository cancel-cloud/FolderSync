// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NumberInput } from "./number-input";

afterEach(cleanup);

test("external values replace an empty draft without emitting a numeric change", async () => {
  const change = vi.fn();
  const user = userEvent.setup();
  const { rerender } = render(
    <NumberInput aria-label="Count" value={20} onChange={change} />,
  );
  const input = screen.getByRole("spinbutton") as HTMLInputElement;
  await user.clear(input);
  expect(input.value).toBe("");
  expect(change).not.toHaveBeenCalled();
  rerender(<NumberInput aria-label="Count" value={7} onChange={change} />);
  expect(input.value).toBe("7");
});

test("native range and disabled constraints remain in force", () => {
  const { rerender } = render(<NumberInput value={0} min={1} max={16} />);
  const input = screen.getByRole("spinbutton") as HTMLInputElement;
  expect(input.validity.rangeUnderflow).toBe(true);
  rerender(<NumberInput value={17} min={1} max={16} />);
  expect(input.validity.rangeOverflow).toBe(true);
  rerender(<NumberInput value={4} min={1} max={16} disabled />);
  expect(input.validity.valid).toBe(true);
  expect(input.disabled).toBe(true);
});
