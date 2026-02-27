/**
 * AddressDisplay Component Tests
 * 
 * T-031: AddressDisplay component tests
 * Test-First Development - tests written before implementation
 */

import React from 'react'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import '@testing-library/jest-dom'

// Mock i18n
jest.mock('@/i18n', () => ({
  useLocale: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'address.copied': 'Copied!',
        'address.clickToCopy': 'Click to copy full address',
        'address.copyFailed': 'Copy failed',
      }
      return translations[key] || key
    },
    locale: 'en',
  }),
}))

// Mock clipboard helper
const mockCopyToClipboard = jest.fn()
jest.mock('@/lib/clipboard', () => ({
  copyToClipboard: (...args: unknown[]) => mockCopyToClipboard(...args),
}))

// Import after mocks
import AddressDisplay from '@/components/AddressDisplay'

describe('AddressDisplay Component', () => {
  const TEST_ADDRESS = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'
  const TEST_SHORT = '5Grwva...utQY'
  const TEST_NICKNAME = 'alice_anarchy'

  beforeEach(() => {
    jest.clearAllMocks()
    mockCopyToClipboard.mockResolvedValue({ success: true, method: 'clipboard' })
  })

  // ============================================================================
  // T-031a: Basic Rendering (FR-009)
  // ============================================================================

  describe('Basic Rendering', () => {
    it('should render short address format by default', () => {
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      expect(screen.getByText(TEST_SHORT)).toBeInTheDocument()
    })

    it('should render short address for valid AccountId', () => {
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const displayedText = screen.getByText(/5Grwva.*utQY/)
      expect(displayedText).toBeInTheDocument()
    })

    it('should handle empty address gracefully', () => {
      render(<AddressDisplay address="" />)
      
      // Should render something (empty or placeholder)
      const container = screen.getByTestId('address-display')
      expect(container).toBeInTheDocument()
    })

    it('should handle very short address', () => {
      render(<AddressDisplay address="5Gr" />)
      
      // Should render the full address if too short to shorten
      expect(screen.getByText('5Gr')).toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-031b: Nickname Display (FR-013)
  // ============================================================================

  describe('Nickname Display', () => {
    it('should display nickname when provided', () => {
      render(<AddressDisplay address={TEST_ADDRESS} nickname={TEST_NICKNAME} />)
      
      expect(screen.getByText(TEST_NICKNAME)).toBeInTheDocument()
    })

    it('should show nickname as primary display', () => {
      render(<AddressDisplay address={TEST_ADDRESS} nickname={TEST_NICKNAME} />)
      
      // Nickname should be visible
      const nicknameElement = screen.getByText(TEST_NICKNAME)
      expect(nicknameElement).toBeInTheDocument()
    })

    it('should show short address alongside nickname', () => {
      render(<AddressDisplay address={TEST_ADDRESS} nickname={TEST_NICKNAME} showAddressWithNickname />)
      
      expect(screen.getByText(TEST_NICKNAME)).toBeInTheDocument()
      expect(screen.getByText(/5Grwva.*utQY/)).toBeInTheDocument()
    })

    it('should fall back to short address when nickname is empty', () => {
      render(<AddressDisplay address={TEST_ADDRESS} nickname="" />)
      
      expect(screen.getByText(TEST_SHORT)).toBeInTheDocument()
    })

    it('should fall back to short address when nickname is undefined', () => {
      render(<AddressDisplay address={TEST_ADDRESS} nickname={undefined} />)
      
      expect(screen.getByText(TEST_SHORT)).toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-031c: Clipboard Copy (FR-010, FR-011)
  // ============================================================================

  describe('Clipboard Copy', () => {
    it('should copy full address to clipboard on click', async () => {
      const user = userEvent.setup()
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      await user.click(element)
      
      expect(mockCopyToClipboard).toHaveBeenCalledWith(TEST_ADDRESS)
    })

    it('should show "Copied!" feedback after successful copy', async () => {
      const user = userEvent.setup()
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      await user.click(element)
      
      await waitFor(() => {
        expect(screen.getByText('Copied!')).toBeInTheDocument()
      })
    })

    it('should hide "Copied!" feedback after a delay', async () => {
      jest.useFakeTimers()
      const user = userEvent.setup({ advanceTimers: jest.advanceTimersByTime })
      
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      await user.click(element)
      
      // Copied message should appear
      await waitFor(() => {
        expect(screen.getByText('Copied!')).toBeInTheDocument()
      })
      
      // Fast-forward 2 seconds
      jest.advanceTimersByTime(2000)
      
      // Copied message should disappear
      await waitFor(() => {
        expect(screen.queryByText('Copied!')).not.toBeInTheDocument()
      })
      
      jest.useRealTimers()
    })

    it('should handle clipboard API failure gracefully', async () => {
      mockCopyToClipboard.mockResolvedValue({ success: false, method: 'manual', error: 'clipboard.unavailable' })
      const user = userEvent.setup()
      
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      await user.click(element)
      
      // Should show error message
      await waitFor(() => {
        expect(screen.getByText('Copy failed')).toBeInTheDocument()
      })
    })

    it('should call onCopy callback when provided', async () => {
      const onCopy = jest.fn()
      const user = userEvent.setup()
      
      render(<AddressDisplay address={TEST_ADDRESS} onCopy={onCopy} />)
      
      const element = screen.getByTestId('address-display')
      await user.click(element)
      
      await waitFor(() => {
        expect(onCopy).toHaveBeenCalledWith(TEST_ADDRESS)
      })
    })
  })

  // ============================================================================
  // T-031d: Tooltip (FR-012)
  // ============================================================================

  describe('Tooltip', () => {
    it('should show tooltip with full address on hover', async () => {
      const user = userEvent.setup()
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      await user.hover(element)
      
      await waitFor(() => {
        // Tooltip should contain full address
        expect(screen.getByText(TEST_ADDRESS)).toBeInTheDocument()
      })
    })

    it('should hide tooltip when mouse leaves', async () => {
      const user = userEvent.setup()
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      await user.hover(element)
      
      // Tooltip appears
      await waitFor(() => {
        expect(screen.getByText(TEST_ADDRESS)).toBeInTheDocument()
      })
      
      // Mouse leaves
      await user.unhover(element)
      
      // Tooltip should disappear
      await waitFor(() => {
        expect(screen.queryByText(TEST_ADDRESS)).not.toBeInTheDocument()
      })
    })

    it('should include click instruction in tooltip', async () => {
      const user = userEvent.setup()
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      await user.hover(element)
      
      await waitFor(() => {
        expect(screen.getByText(/Click to copy/)).toBeInTheDocument()
      })
    })
  })

  // ============================================================================
  // T-031e: Accessibility
  // ============================================================================

  describe('Accessibility', () => {
    it('should have appropriate ARIA role', () => {
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      expect(element).toHaveAttribute('role', 'button')
    })

    it('should have accessible name', () => {
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      expect(element).toHaveAccessibleName()
    })

    it('should be keyboard accessible', async () => {
      const user = userEvent.setup()
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      element.focus()
      
      await user.keyboard('{Enter}')
      
      expect(mockCopyToClipboard).toHaveBeenCalledWith(TEST_ADDRESS)
    })

    it('should respond to space key', async () => {
      const user = userEvent.setup()
      render(<AddressDisplay address={TEST_ADDRESS} />)
      
      const element = screen.getByTestId('address-display')
      element.focus()
      
      await user.keyboard(' ')
      
      expect(mockCopyToClipboard).toHaveBeenCalledWith(TEST_ADDRESS)
    })
  })

  // ============================================================================
  // T-031f: Styling Variants
  // ============================================================================

  describe('Styling Variants', () => {
    it('should support compact size variant', () => {
      render(<AddressDisplay address={TEST_ADDRESS} size="compact" />)
      
      const element = screen.getByTestId('address-display')
      expect(element).toHaveClass(/compact/)
    })

    it('should support full size variant', () => {
      render(<AddressDisplay address={TEST_ADDRESS} size="full" />)
      
      const element = screen.getByTestId('address-display')
      expect(element).toHaveClass(/full/)
    })

    it('should apply custom className', () => {
      render(<AddressDisplay address={TEST_ADDRESS} className="custom-class" />)
      
      const element = screen.getByTestId('address-display')
      expect(element).toHaveClass('custom-class')
    })
  })
})
