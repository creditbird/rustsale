import { expect, test } from "nsh/test";
import { facebookTokenFromInput } from "./stores/facebookToken";

test("a bare token is taken as written", () => {
  expect(facebookTokenFromInput("  EAAabc123  ")).toBe("EAAabc123");
});

test("the token is read from an OAuth redirect fragment", () => {
  expect(
    facebookTokenFromInput(
      "https://facebook.com/connect/login_success.html#access_token=EAAxyz&state=abc&expires_in=5184000"
    )
  ).toBe("EAAxyz");
});

test("a long-lived token wins over a plain access token", () => {
  expect(
    facebookTokenFromInput(
      "https://example.test/cb?long_lived_token=LL1&access_token=SHORT"
    )
  ).toBe("LL1");
});

test("an encoded token is decoded once", () => {
  expect(
    facebookTokenFromInput("https://example.test/cb#provider_token=a%2Bb%3Dc")
  ).toBe("a+b=c");
});

test("nothing usable reads as missing", () => {
  expect(facebookTokenFromInput("")).toBeNull();
  expect(facebookTokenFromInput("   ")).toBeNull();
  expect(facebookTokenFromInput("https://example.test/cb?state=only")).toBeNull();
});
