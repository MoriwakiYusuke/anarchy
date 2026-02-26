/**
 * NicknameSettings Component Tests
 * 
 * T-039: NicknameSettings component tests
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
        'nickname.title': 'Nickname',
        'nickname.set': 'Set',
        'nickname.clear': 'Clear',
        'nickname.placeholder': 'Enter nickname...',
        'nickname.setting': 'Setting...',
        'nickname.success': 'Nickname set',
        'nickname.cleared': 'Nickname cleared',
        'nickname.error': 'Failed to set nickname',
        'error.nicknameTooLong': 'Nickname is too long (max 128 bytes)',
        'error.nicknameEmpty': 'Please enter a nickname',
        'name.label': 'Name',
        'name.change': 'Change',
      }
      return translations[key] || key
    },
    locale: 'en',
  }),
}))

// Mock useNickname hook
const mockSetNickname = jest.fn()
const mockClearNickname = jest.fn()
const mockRefetch = jest.fn()

jest.mock('@/hooks/useNickname', () => ({
  useNickname: jest.fn(() => ({
    nickname: null,
    isLoading: false,
    error: null,
    state: { status: 'idle' },
    setNickname: mockSetNickname,
    clearNickname: mockClearNickname,
    refetch: mockRefetch,
  })),
}))

// Import after mocks
import NicknameSettings from '@/components/NicknameSettings'
import { useNickname } from '@/hooks/useNickname'

describe('NicknameSettings Component', () => {
  const TEST_NICKNAME = 'alice_anarchy'

  beforeEach(() => {
    jest.clearAllMocks()
    mockSetNickname.mockResolvedValue(undefined)
    mockClearNickname.mockResolvedValue(undefined)
    ;(useNickname as jest.Mock).mockReturnValue({
      nickname: null,
      isLoading: false,
      error: null,
      state: { status: 'idle' },
      setNickname: mockSetNickname,
      clearNickname: mockClearNickname,
      refetch: mockRefetch,
    })
  })

  // ============================================================================
  // T-039a: Basic Rendering
  // ============================================================================

  // Helper to expand the collapsible form
  const expandForm = async (user: ReturnType<typeof userEvent.setup>) => {
    const changeButton = screen.getByRole('button', { name: /change/i })
    await user.click(changeButton)
  }

  describe('Basic Rendering', () => {
    it('should render nickname input field', async () => {
      const user = userEvent.setup()
      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      expect(screen.getByPlaceholderText('Enter nickname...')).toBeInTheDocument()
    })

    it('should render set button', async () => {
      const user = userEvent.setup()
      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      expect(screen.getByRole('button', { name: /set/i })).toBeInTheDocument()
    })

    it('should show current nickname in input if set', async () => {
      const user = userEvent.setup()
      ;(useNickname as jest.Mock).mockReturnValue({
        nickname: TEST_NICKNAME,
        isLoading: false,
        error: null,
        state: { status: 'idle' },
        setNickname: mockSetNickname,
        clearNickname: mockClearNickname,
        refetch: mockRefetch,
      })

      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      expect(screen.getByDisplayValue(TEST_NICKNAME)).toBeInTheDocument()
    })

    it('should show clear button when nickname is set', async () => {
      const user = userEvent.setup()
      ;(useNickname as jest.Mock).mockReturnValue({
        nickname: TEST_NICKNAME,
        isLoading: false,
        error: null,
        state: { status: 'idle' },
        setNickname: mockSetNickname,
        clearNickname: mockClearNickname,
        refetch: mockRefetch,
      })

      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      expect(screen.getByRole('button', { name: /clear/i })).toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-039b: Set Nickname Interaction
  // ============================================================================

  describe('Set Nickname', () => {
    it('should call setNickname when form is submitted', async () => {
      const user = userEvent.setup()
      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      const input = screen.getByPlaceholderText('Enter nickname...')
      await user.type(input, TEST_NICKNAME)
      
      const setButton = screen.getByRole('button', { name: /set/i })
      await user.click(setButton)
      
      expect(mockSetNickname).toHaveBeenCalledWith(TEST_NICKNAME)
    })

    it('should disable button while setting', async () => {
      const user = userEvent.setup()
      ;(useNickname as jest.Mock).mockReturnValue({
        nickname: null,
        isLoading: false,
        error: null,
        state: { status: 'pending' },
        setNickname: mockSetNickname,
        clearNickname: mockClearNickname,
        refetch: mockRefetch,
      })

      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      const setButton = screen.getByRole('button', { name: /setting/i })
      expect(setButton).toBeDisabled()
    })

    it('should show loading state while pending', async () => {
      const user = userEvent.setup()
      ;(useNickname as jest.Mock).mockReturnValue({
        nickname: null,
        isLoading: false,
        error: null,
        state: { status: 'pending' },
        setNickname: mockSetNickname,
        clearNickname: mockClearNickname,
        refetch: mockRefetch,
      })

      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      expect(screen.getByText('Setting...')).toBeInTheDocument()
    })

    it('should show success message after successful set', async () => {
      const user = userEvent.setup()
      ;(useNickname as jest.Mock).mockReturnValue({
        nickname: TEST_NICKNAME,
        isLoading: false,
        error: null,
        state: { status: 'success' },
        setNickname: mockSetNickname,
        clearNickname: mockClearNickname,
        refetch: mockRefetch,
      })

      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      expect(screen.getByText('Nickname set')).toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-039c: Clear Nickname Interaction
  // ============================================================================

  describe('Clear Nickname', () => {
    it('should call clearNickname when clear button is clicked', async () => {
      const user = userEvent.setup()
      ;(useNickname as jest.Mock).mockReturnValue({
        nickname: TEST_NICKNAME,
        isLoading: false,
        error: null,
        state: { status: 'idle' },
        setNickname: mockSetNickname,
        clearNickname: mockClearNickname,
        refetch: mockRefetch,
      })

      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      const clearButton = screen.getByRole('button', { name: /clear/i })
      await user.click(clearButton)
      
      expect(mockClearNickname).toHaveBeenCalled()
    })

    it('should show cleared message after successful clear', async () => {
      const user = userEvent.setup()
      ;(useNickname as jest.Mock).mockReturnValue({
        nickname: null,
        isLoading: false,
        error: null,
        state: { status: 'success' },
        setNickname: mockSetNickname,
        clearNickname: mockClearNickname,
        refetch: mockRefetch,
      })

      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      // Success state with no nickname means it was cleared
      await expandForm(user)
      expect(screen.getByText('Nickname cleared')).toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-039d: Error Handling
  // ============================================================================

  describe('Error Handling', () => {
    it('should display error message', async () => {
      const user = userEvent.setup()
      ;(useNickname as jest.Mock).mockReturnValue({
        nickname: null,
        isLoading: false,
        error: 'error.nicknameTooLong',
        state: { status: 'error' },
        setNickname: mockSetNickname,
        clearNickname: mockClearNickname,
        refetch: mockRefetch,
      })

      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      // Error message is translated
      expect(screen.getByText('Nickname is too long (max 128 bytes)')).toBeInTheDocument()
    })

    it('should show error styling on error state', async () => {
      const user = userEvent.setup()
      ;(useNickname as jest.Mock).mockReturnValue({
        nickname: null,
        isLoading: false,
        error: 'error.nicknameEmpty',
        state: { status: 'error' },
        setNickname: mockSetNickname,
        clearNickname: mockClearNickname,
        refetch: mockRefetch,
      })

      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      // Error message is translated to 'Please enter a nickname'
      const errorMessage = screen.getByText('Please enter a nickname')
      expect(errorMessage).toHaveClass(/error/)
    })
  })

  // ============================================================================
  // T-039e: Validation
  // ============================================================================

  describe('Validation', () => {
    it('should show character count', async () => {
      const user = userEvent.setup()
      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      const input = screen.getByPlaceholderText('Enter nickname...')
      await user.type(input, 'alice')
      
      // Should show byte count
      expect(screen.getByText(/5.*128/)).toBeInTheDocument()
    })

    it('should warn when approaching limit', async () => {
      const user = userEvent.setup()
      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      const input = screen.getByPlaceholderText('Enter nickname...')
      // Type 120 characters (close to 128 byte limit)
      await user.type(input, 'a'.repeat(120))
      
      // Counter should have warning styling
      expect(screen.getByText(/120.*128/)).toHaveClass(/warning/)
    })
  })

  // ============================================================================
  // T-039f: Accessibility
  // ============================================================================

  describe('Accessibility', () => {
    it('should have proper form labels', async () => {
      const user = userEvent.setup()
      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      const input = screen.getByPlaceholderText('Enter nickname...')
      expect(input).toHaveAccessibleName()
    })

    it('should support form submission via Enter key', async () => {
      const user = userEvent.setup()
      render(<NicknameSettings client={{}} unsafeApi={{}} accountId="5Grw..." signer={{}} />)
      
      await expandForm(user)
      const input = screen.getByPlaceholderText('Enter nickname...')
      await user.type(input, TEST_NICKNAME)
      await user.keyboard('{Enter}')
      
      expect(mockSetNickname).toHaveBeenCalledWith(TEST_NICKNAME)
    })
  })
})
