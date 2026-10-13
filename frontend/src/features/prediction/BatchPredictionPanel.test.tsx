import { expect, it } from "vitest";
import { parseBatchJson } from "./BatchPredictionPanel";
it("maps named batch inputs into model order and rejects incomplete or nonnumeric rows", () => {
  expect(parseBatchJson('[{"temperature":900,"C":0.2}]',["C","temperature"])).toEqual([[.2,900]]);
  expect(() => parseBatchJson('[{"C":0.2}]',["C","temperature"])).toThrow("第 1 行");
  expect(() => parseBatchJson('[["0.2",900]]',["C","temperature"])).toThrow();
  expect(() => parseBatchJson('[]',["C"])).toThrow();
});
