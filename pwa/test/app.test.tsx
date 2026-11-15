import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App";
import { loadHistory, loadPrefs } from "../src/storage";

const RESULT = {
  detected_language: "romanized_nepali",
  nepali_script: "मलाई थाहा छैन।",
  accurate_english: "I don't know.",
  styled_english: "I'm not sure.",
  grammar_notes: [],
  word_meanings: [{ source: "thaha chaina", devanagari: "थाहा छैन", meaning: "don't know", literal: "knowledge is not", note: "negative of thaha cha", uncertain: false }],
  ambiguities: [],
  warnings: [],
  mode: "accurate",
  model: "gemini-test",
  quota: { used: 1, limit: 150, resetAt: Date.now() + 1e6 },
};

type Handler = (url: string, init?: RequestInit) => Response | Promise<Response>;
const ok = (b: unknown, status = 200) => new Response(JSON.stringify(b), { status, headers: { "Content-Type": "application/json" } });
function mockFetch(h: Handler) {
  const f = vi.fn(async (u: RequestInfo | URL, init?: RequestInit) => h(String(u), init));
  vi.stubGlobal("fetch", f);
  return f;
}
const signedIn = () => localStorage.setItem("dn.session", JSON.stringify({ token: "v1.test.sig", expiresAt: Date.now() + 86400_000 }));

beforeEach(() => {
  localStorage.clear();
  Object.defineProperty(navigator, "onLine", { configurable: true, value: true });
});
afterEach(() => vi.unstubAllGlobals());

describe("sign in", () => {
  it("shows the private sign-in screen when signed out and ships no credentials", () => {
    render(<App />);
    expect(screen.getByRole("button", { name: "Sign in" })).toBeDisabled();
    expect(screen.getByPlaceholderText("Access code")).toHaveAttribute("type", "password");
  });

  it("exchanges the access code for a stored device token", async () => {
    const f = mockFetch((u) => (u.endsWith("/auth/login") ? ok({ token: "v1.abc.def", expiresAt: Date.now() + 1e9 }) : ok({ expiresAt: 0, quota: RESULT.quota })));
    render(<App />);
    await userEvent.type(screen.getByPlaceholderText("Access code"), "my-private-code");
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    await screen.findByRole("button", { name: "Translate" });
    expect(JSON.parse(localStorage.getItem("dn.session")!).token).toBe("v1.abc.def");
    const [url, init] = f.mock.calls[0]!;
    expect(url).toBe("https://api.test/auth/login");
    expect(init?.credentials).toBe("omit");
  });

  it("shows the server's message for a wrong code", async () => {
    mockFetch(() => ok({ error: { code: "bad_code", message: "That access code is not correct." } }, 401));
    render(<App />);
    await userEvent.type(screen.getByPlaceholderText("Access code"), "nope");
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("not correct");
  });
});

describe("translating", () => {
  it("sends the token, shows English, Devanagari and word meanings", async () => {
    signedIn();
    const f = mockFetch((u) => (u.endsWith("/translate") ? ok(RESULT) : ok({ expiresAt: 0, quota: RESULT.quota })));
    render(<App />);
    await userEvent.type(screen.getByLabelText("Text to translate"), "Malai thaha chaina");
    await userEvent.click(screen.getByRole("button", { name: "Translate" }));
    expect(await screen.findByText("I don't know.")).toBeInTheDocument();
    expect(screen.getByText("मलाई थाहा छैन।")).toBeInTheDocument();
    expect(screen.getByText("thaha chaina")).toBeInTheDocument();
    const call = f.mock.calls.find(([u]) => String(u).endsWith("/translate"))!;
    expect(new Headers(call[1]?.headers).get("Authorization")).toBe("Bearer v1.test.sig");
    expect(JSON.parse(String(call[1]?.body))).toEqual({ text: "Malai thaha chaina", mode: "accurate", meanings: true });
  });

  it("copies the translation with one tap", async () => {
    signedIn();
    mockFetch((u) => (u.endsWith("/translate") ? ok(RESULT) : ok({ expiresAt: 0, quota: RESULT.quota })));
    const user = userEvent.setup();
    const write = vi.spyOn(navigator.clipboard, "writeText");
    render(<App />);
    await user.type(screen.getByLabelText("Text to translate"), "Malai thaha chaina");
    await user.click(screen.getByRole("button", { name: "Translate" }));
    await screen.findByText("I don't know.");
    await user.click(screen.getByRole("button", { name: /^Copy$/ }));
    expect(write).toHaveBeenCalledWith("I don't know.");
    expect(await screen.findByText("Copied")).toBeInTheDocument();
  });

  it("does not save history unless the user turns it on", async () => {
    signedIn();
    mockFetch((u) => (u.endsWith("/translate") ? ok(RESULT) : ok({ expiresAt: 0, quota: RESULT.quota })));
    expect(loadPrefs().history).toBe(false);
    render(<App />);
    await userEvent.type(screen.getByLabelText("Text to translate"), "Malai thaha chaina");
    await userEvent.click(screen.getByRole("button", { name: "Translate" }));
    await screen.findByText("I don't know.");
    expect(loadHistory()).toEqual([]);
  });

  it("signs out when the token is rejected", async () => {
    signedIn();
    mockFetch((u) => (u.endsWith("/translate") ? ok({ error: { code: "signed_out", message: "Please sign in." } }, 401) : ok({ expiresAt: 0, quota: RESULT.quota })));
    render(<App />);
    await userEvent.type(screen.getByLabelText("Text to translate"), "namaste");
    await userEvent.click(screen.getByRole("button", { name: "Translate" }));
    expect(await screen.findByRole("button", { name: "Sign in" })).toBeInTheDocument();
    expect(localStorage.getItem("dn.session")).toBeNull();
  });

  it("explains clearly when offline instead of calling the API", async () => {
    signedIn();
    const f = mockFetch(() => ok({ expiresAt: 0, quota: RESULT.quota }));
    render(<App />);
    await userEvent.type(screen.getByLabelText("Text to translate"), "namaste");
    Object.defineProperty(navigator, "onLine", { configurable: true, value: false });
    act(() => { window.dispatchEvent(new Event("offline")); });
    await userEvent.click(screen.getByRole("button", { name: "Translate" }));
    await waitFor(() => expect(screen.getAllByText(/You're offline/).length).toBeGreaterThan(0));
    expect(f.mock.calls.some(([u]) => String(u).endsWith("/translate"))).toBe(false);
  });

  it("shows the daily-limit message from the server", async () => {
    signedIn();
    mockFetch((u) => (u.endsWith("/translate") ? ok({ error: { code: "daily_limit", message: "Today's translation limit is used up." } }, 429) : ok({ expiresAt: 0, quota: RESULT.quota })));
    render(<App />);
    await userEvent.type(screen.getByLabelText("Text to translate"), "namaste");
    await userEvent.click(screen.getByRole("button", { name: "Translate" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("limit is used up");
  });
});
