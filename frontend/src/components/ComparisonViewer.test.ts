import { describe, expect, test } from "bun:test";
import { imageFitScale, nextZoom } from "./ComparisonViewer";

describe("image viewer zoom steps", () => {
  test("uses the same image-inspection presets in either direction", () => {
    expect(nextZoom(1, true)).toBe(1.1);
    expect(nextZoom(1.1, false)).toBe(1);
    expect(nextZoom(0.6, true)).toBe(2 / 3);
    expect(nextZoom(0.6, false)).toBe(0.5);
    expect(nextZoom(5, true)).toBe(6);
    expect(nextZoom(6, true)).toBe(8);
  });

  test("keeps fitted images below the minimum and clamps at either limit", () => {
    expect(nextZoom(0.1, false)).toBe(0.1);
    expect(nextZoom(0.1, true)).toBe(0.25);
    expect(nextZoom(0.25, false)).toBe(0.25);
    expect(nextZoom(8, true)).toBe(8);
    expect(nextZoom(8, false)).toBe(6);
  });

  test("does not get stuck on rounded fit percentages", () => {
    expect(nextZoom(0.9999, true)).toBe(1.1);
    expect(nextZoom(1.0001, false)).toBe(0.9);
  });

  test("fits tall images to the window or width without upscaling", () => {
    const image = { width: 1200, height: 6000 };
    const viewport = { width: 600, height: 600 };
    expect(imageFitScale(image, viewport, "width")).toBe(0.5);
    expect(imageFitScale(image, viewport, "area")).toBe(0.1);
    expect(imageFitScale({ width: 10, height: 10 }, viewport, "area")).toBe(1);
    expect(imageFitScale(image, { width: 0, height: 0 }, "area")).toBe(0.001);
  });
});
