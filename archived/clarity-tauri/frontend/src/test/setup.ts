import { vi } from "vitest";
import "@testing-library/jest-dom/vitest";

// Mock matchMedia for jsdom environment (theme detection)
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

// Mock scrollIntoView (auto-scroll in chat)
Element.prototype.scrollIntoView = vi.fn();
