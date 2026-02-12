import '@testing-library/jest-dom';
import { TextEncoder, TextDecoder } from 'util';

// Polyfill for TextEncoder/TextDecoder (Node.js環境用)
global.TextEncoder = TextEncoder;
global.TextDecoder = TextDecoder as typeof global.TextDecoder;

// Mock matchMedia for useReducedMotion hook tests
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: jest.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: jest.fn(),
    removeListener: jest.fn(),
    addEventListener: jest.fn(),
    removeEventListener: jest.fn(),
    dispatchEvent: jest.fn(),
  })),
});

// Mock localStorage for i18n persistence tests
const localStorageMock = {
  getItem: jest.fn(),
  setItem: jest.fn(),
  removeItem: jest.fn(),
  clear: jest.fn(),
};
Object.defineProperty(window, 'localStorage', {
  value: localStorageMock,
});
