import type { FlowStep } from "./api.ts";

/** A step in words. */
export function describeStep(step: FlowStep): string {
  switch (step.type) {
    case "launch":
      return "Start the app";
    case "tap": {
      const name = step.label?.text ?? step.label?.id;
      return name ? `Tap “${name}”` : `Tap at ${step.x}, ${step.y}`;
    }
    case "longPress": {
      const name = step.label?.text ?? step.label?.id;
      return name ? `Press and hold “${name}”` : `Press and hold at ${step.x}, ${step.y}`;
    }
    case "swipe":
      return `Swipe from ${step.x1}, ${step.y1} to ${step.x2}, ${step.y2}`;
    case "text":
      return `Type “${step.text}”`;
    case "key":
      return `Press ${step.key.replaceAll("_", " ")}`;
    case "wait":
      return step.ms >= 1000 ? `Wait ${(step.ms / 1000).toFixed(1).replace(/\.0$/, "")} s` : `Wait ${step.ms} ms`;
  }
}
