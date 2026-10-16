// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PathPair } from "./path-pair";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

test("copies the selected side's complete UNC path without normalization", async () => {
  const user = userEvent.setup();
  const clipboard = vi
    .spyOn(navigator.clipboard, "writeText")
    .mockResolvedValue();
  render(
    <PathPair
      a={"C:\\owned\\A"}
      b={"\\\\server\\share\\東京\\trailing space "}
      lang="en"
    />,
  );
  await user.click(screen.getByRole("button", { name: "Copy folder B path" }));
  expect(clipboard).toHaveBeenCalledWith(
    "\\\\server\\share\\東京\\trailing space ",
  );
  expect(screen.getByRole("status").textContent).toBe("Path copied");
  expect(screen.queryByRole("textbox")).toBeNull();
});

test("failed clipboard access exposes a selectable full path and never reports success", async () => {
  const user = userEvent.setup();
  vi.spyOn(navigator.clipboard, "writeText").mockRejectedValue(
    new Error("denied"),
  );
  render(<PathPair a={"C:\\owned\\東京"} b={"C:\\owned\\B"} lang="de" />);
  await user.click(
    screen.getByRole("button", { name: "Pfad von Ordner A kopieren" }),
  );
  const input = screen.getByRole("textbox", {
    name: "Vollständiger Pfad von Ordner A",
  }) as HTMLTextAreaElement;
  expect(input.value).toBe("C:\\owned\\東京");
  expect(input.readOnly).toBe(true);
  expect(screen.getByRole("status").textContent).toContain("manuell kopieren");
  await user.click(
    screen.getByRole("button", { name: "Pfad von Ordner A einklappen" }),
  );
  expect(screen.queryByRole("textbox")).toBeNull();
  await user.click(
    screen.getByRole("button", { name: "Pfad von Ordner B aufklappen" }),
  );
  expect(
    (
      screen.getByRole("textbox", {
        name: "Vollständiger Pfad von Ordner B",
      }) as HTMLTextAreaElement
    ).value,
  ).toBe("C:\\owned\\B");
});
