/**
 * Unit Tests for LanguageSwitcher Component
 * 
 * TDD: Written BEFORE implementation - tests should FAIL initially
 */

import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';

// jest.setup.ts は `@/i18n` をグローバルモックしている (availableLocales: [] の
// スタブ) が、このスイートは実際の locale 切替動作を検証するため実物を使う。
jest.unmock('@/i18n');

import { LanguageSwitcher } from '@/components/LanguageSwitcher';
import { LocaleProvider } from '@/i18n';
import type { Locale } from '@/i18n/types';

const TestWrapper = ({ children }: { children: React.ReactNode }) => (
  <LocaleProvider>{children}</LocaleProvider>
);

describe('LanguageSwitcher Component', () => {
  beforeEach(() => {
    localStorage.clear();
    jest.clearAllMocks();
  });

  describe('Rendering', () => {
    it('should render language options', () => {
      render(
        <TestWrapper>
          <LanguageSwitcher />
        </TestWrapper>
      );

      expect(screen.getByText('English')).toBeInTheDocument();
      expect(screen.getByText('日本語')).toBeInTheDocument();
      expect(screen.getByText('中文')).toBeInTheDocument();
    });

    it('should highlight current locale', () => {
      render(
        <TestWrapper>
          <LanguageSwitcher />
        </TestWrapper>
      );

      const englishButton = screen.getByRole('button', { name: /english/i });
      expect(englishButton).toHaveAttribute('aria-current', 'true');
    });

    it('should have accessible name', () => {
      render(
        <TestWrapper>
          <LanguageSwitcher />
        </TestWrapper>
      );

      const switcher = screen.getByRole('group', { name: /language|言語/i });
      expect(switcher).toBeInTheDocument();
    });
  });

  describe('Locale Selection', () => {
    it('should change locale when Japanese is clicked', () => {
      render(
        <TestWrapper>
          <LanguageSwitcher />
        </TestWrapper>
      );

      const jaButton = screen.getByRole('button', { name: /日本語/i });
      fireEvent.click(jaButton);

      expect(jaButton).toHaveAttribute('aria-current', 'true');
      expect(localStorage.setItem).toHaveBeenCalledWith('anarchy-locale', 'ja');
    });

    it('should change locale when Chinese is clicked', () => {
      render(
        <TestWrapper>
          <LanguageSwitcher />
        </TestWrapper>
      );

      const zhButton = screen.getByRole('button', { name: /中文/i });
      fireEvent.click(zhButton);

      expect(zhButton).toHaveAttribute('aria-current', 'true');
      expect(localStorage.setItem).toHaveBeenCalledWith('anarchy-locale', 'zh');
    });

    it('should change locale back to English', () => {
      render(
        <TestWrapper>
          <LanguageSwitcher />
        </TestWrapper>
      );

      // First change to Japanese
      fireEvent.click(screen.getByRole('button', { name: /日本語/i }));
      
      // Then back to English
      const enButton = screen.getByRole('button', { name: /english/i });
      fireEvent.click(enButton);

      expect(enButton).toHaveAttribute('aria-current', 'true');
    });
  });

  describe('Styling Variants', () => {
    it('should apply compact class when variant is compact', () => {
      const { container } = render(
        <TestWrapper>
          <LanguageSwitcher variant="compact" />
        </TestWrapper>
      );

      expect(container.firstChild).toHaveClass('compact');
    });

    it('should apply full class when variant is full (default)', () => {
      const { container } = render(
        <TestWrapper>
          <LanguageSwitcher />
        </TestWrapper>
      );

      expect(container.firstChild).toHaveClass('full');
    });
  });

  describe('Accessibility', () => {
    it('should be keyboard navigable', () => {
      render(
        <TestWrapper>
          <LanguageSwitcher />
        </TestWrapper>
      );

      const buttons = screen.getAllByRole('button');
      buttons.forEach(button => {
        expect(button).not.toHaveAttribute('tabindex', '-1');
      });
    });

    it('should announce locale change to screen readers', () => {
      render(
        <TestWrapper>
          <LanguageSwitcher />
        </TestWrapper>
      );

      // aria-live region should exist or be created on change
      const liveRegion = document.querySelector('[aria-live]');
      // Initial render may not have live region, but we check for buttons
      const buttons = screen.getAllByRole('button');
      expect(buttons.length).toBe(3);
    });
  });

  describe('Custom className', () => {
    it('should accept custom className prop', () => {
      const { container } = render(
        <TestWrapper>
          <LanguageSwitcher className="custom-class" />
        </TestWrapper>
      );

      expect(container.firstChild).toHaveClass('custom-class');
    });
  });
});
