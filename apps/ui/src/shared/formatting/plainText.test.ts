import { expect, it } from "vitest";
import { plainText } from "./plainText";

it("reduces markdown to its words", () => {
  expect(plainText("## Alpha Squad - Metropolitan **Special** Response")).toBe("Alpha Squad - Metropolitan Special Response");
  expect(plainText("Name: *Maria*\n- Age: 31\n> quiet [link](https://x.y) `code` ![pic](a.png)")).toBe("Name: Maria Age: 31 quiet link code pic");
  expect(plainText("snake_case and 2 * 3 stay")).toBe("snake_case and 2 * 3 stay");
});
