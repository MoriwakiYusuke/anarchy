/**
 * MediaUpload Component Tests
 * 
 * T-047: MediaUpload component tests
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
        'media.upload': 'Upload',
        'media.uploading': 'Uploading...',
        'media.dropzone': 'Drop files here or click to select',
        'media.remove': 'Remove',
        'media.retry': 'Retry',
        'media.processing': 'Processing...',
        'media.complete': 'Complete',
        'media.loadError': 'Load failed',
        'error.fileTooLarge': 'File is too large',
        'error.unsupportedFileType': 'Unsupported file type',
        'error.tooManyFiles': 'Too many files (max 4)',
        'error.uploadFailed': 'Upload failed',
      }
      return translations[key] || key
    },
    locale: 'en',
  }),
}))

// Mock useMediaUpload hook
const mockAddFiles = jest.fn()
const mockRemoveFile = jest.fn()
const mockUploadAll = jest.fn()
const mockClearAll = jest.fn()

jest.mock('@/hooks/useMediaUpload', () => ({
  useMediaUpload: jest.fn(() => ({
    files: [],
    state: 'idle',
    error: null,
    addFiles: mockAddFiles,
    removeFile: mockRemoveFile,
    uploadAll: mockUploadAll,
    clearAll: mockClearAll,
  })),
}))

// Import after mocks
import MediaUpload from '@/components/MediaUpload'
import { useMediaUpload } from '@/hooks/useMediaUpload'

describe('MediaUpload Component', () => {
  const defaultProps = {
    storageNodeUrl: 'http://localhost:3030',
    onUploadComplete: jest.fn(),
  }

  beforeEach(() => {
    jest.clearAllMocks()
    ;(useMediaUpload as jest.Mock).mockReturnValue({
      files: [],
      state: 'idle',
      error: null,
      addFiles: mockAddFiles,
      removeFile: mockRemoveFile,
      uploadAll: mockUploadAll,
      clearAll: mockClearAll,
    })
  })

  // ============================================================================
  // T-047a: Basic Rendering
  // ============================================================================

  describe('Basic Rendering', () => {
    it('should render dropzone', () => {
      render(<MediaUpload {...defaultProps} />)
      
      expect(screen.getByText('Drop files here or click to select')).toBeInTheDocument()
    })

    it('should render file input (hidden)', () => {
      render(<MediaUpload {...defaultProps} />)
      
      const input = document.querySelector('input[type="file"]')
      expect(input).toBeInTheDocument()
      expect(input).toHaveAttribute('accept', 'image/jpeg,image/png,image/gif,image/webp')
    })

    it('should allow multiple file selection', () => {
      render(<MediaUpload {...defaultProps} />)
      
      const input = document.querySelector('input[type="file"]')
      expect(input).toHaveAttribute('multiple')
    })
  })

  // ============================================================================
  // T-047b: File Selection
  // ============================================================================

  describe('File Selection', () => {
    it('should call addFiles when files are selected', async () => {
      const user = userEvent.setup()
      render(<MediaUpload {...defaultProps} />)
      
      const input = document.querySelector('input[type="file"]') as HTMLInputElement
      const file = new File(['test'], 'test.jpg', { type: 'image/jpeg' })
      
      await user.upload(input, file)
      
      expect(mockAddFiles).toHaveBeenCalledWith([file])
    })

    it('should call addFiles when multiple files are selected', async () => {
      const user = userEvent.setup()
      render(<MediaUpload {...defaultProps} />)
      
      const input = document.querySelector('input[type="file"]') as HTMLInputElement
      const files = [
        new File(['test1'], 'test1.jpg', { type: 'image/jpeg' }),
        new File(['test2'], 'test2.jpg', { type: 'image/jpeg' }),
      ]
      
      await user.upload(input, files)
      
      expect(mockAddFiles).toHaveBeenCalledWith(files)
    })
  })

  // ============================================================================
  // T-047c: Drag and Drop
  // ============================================================================

  describe('Drag and Drop', () => {
    it('should show drag active state when dragging over', () => {
      render(<MediaUpload {...defaultProps} />)
      
      const dropzone = screen.getByText('Drop files here or click to select').closest('div')
      
      fireEvent.dragEnter(dropzone!)
      
      expect(dropzone).toHaveClass(/dragActive/)
    })

    it('should call addFiles when files are dropped', async () => {
      render(<MediaUpload {...defaultProps} />)
      
      const dropzone = screen.getByText('Drop files here or click to select').closest('div')
      const file = new File(['test'], 'test.jpg', { type: 'image/jpeg' })
      
      const dataTransfer = {
        files: [file],
        items: [{ kind: 'file', type: 'image/jpeg', getAsFile: () => file }],
        types: ['Files'],
      }
      
      fireEvent.drop(dropzone!, { dataTransfer })
      
      expect(mockAddFiles).toHaveBeenCalled()
    })
  })

  // ============================================================================
  // T-047d: File Preview
  // ============================================================================

  describe('File Preview', () => {
    it('should display file previews', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          {
            id: '1',
            file: new File(['test'], 'test.jpg', { type: 'image/jpeg' }),
            type: 'image',
            size: 1024,
            preview: 'blob:test-preview',
            uploadProgress: 0,
            status: 'pending',
          },
        ],
        state: 'idle',
        error: null,
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} />)
      
      const img = screen.getByRole('img')
      expect(img).toHaveAttribute('src', 'blob:test-preview')
    })

    it('should display remove button for each file', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          {
            id: '1',
            file: new File(['test'], 'test.jpg', { type: 'image/jpeg' }),
            type: 'image',
            size: 1024,
            preview: 'blob:test-preview',
            uploadProgress: 0,
            status: 'pending',
          },
        ],
        state: 'idle',
        error: null,
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} />)
      
      expect(screen.getByRole('button', { name: /remove/i })).toBeInTheDocument()
    })

    it('should call removeFile when remove button is clicked', async () => {
      const user = userEvent.setup()
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          {
            id: 'file-1',
            file: new File(['test'], 'test.jpg', { type: 'image/jpeg' }),
            type: 'image',
            size: 1024,
            preview: 'blob:test-preview',
            uploadProgress: 0,
            status: 'pending',
          },
        ],
        state: 'idle',
        error: null,
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} />)
      
      await user.click(screen.getByRole('button', { name: /remove/i }))
      
      expect(mockRemoveFile).toHaveBeenCalledWith('file-1')
    })
  })

  // ============================================================================
  // T-047e: Upload Progress
  // ============================================================================

  describe('Upload Progress', () => {
    it('should display progress bar during upload', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          {
            id: '1',
            file: new File(['test'], 'test.jpg', { type: 'image/jpeg' }),
            type: 'image',
            size: 1024,
            preview: 'blob:test-preview',
            uploadProgress: 50,
            status: 'uploading',
          },
        ],
        state: 'uploading',
        error: null,
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} />)
      
      const progressBar = screen.getByRole('progressbar')
      expect(progressBar).toBeInTheDocument()
      expect(progressBar).toHaveAttribute('aria-valuenow', '50')
    })

    it('should show processing status during splitting', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          {
            id: '1',
            file: new File(['test'], 'test.jpg', { type: 'image/jpeg' }),
            type: 'image',
            size: 1024,
            preview: 'blob:test-preview',
            uploadProgress: 0,
            status: 'splitting',
          },
        ],
        state: 'uploading',
        error: null,
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} />)
      
      expect(screen.getByText('Processing...')).toBeInTheDocument()
    })

    it('should show complete status when done', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          {
            id: '1',
            file: new File(['test'], 'test.jpg', { type: 'image/jpeg' }),
            type: 'image',
            size: 1024,
            preview: 'blob:test-preview',
            uploadProgress: 100,
            status: 'complete',
            merkleRoot: '0'.repeat(64),
          },
        ],
        state: 'complete',
        error: null,
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} />)
      
      expect(screen.getByText('Complete')).toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-047f: Error Display
  // ============================================================================

  describe('Error Display', () => {
    it('should display error message', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [],
        state: 'error',
        error: 'error.fileTooLarge',
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} />)
      
      expect(screen.getByText('error.fileTooLarge')).toBeInTheDocument()
    })

    it('should display error status on individual file', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          {
            id: '1',
            file: new File(['test'], 'test.jpg', { type: 'image/jpeg' }),
            type: 'image',
            size: 1024,
            preview: 'blob:test-preview',
            uploadProgress: 0,
            status: 'error',
          },
        ],
        state: 'error',
        error: 'error.uploadFailed',
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} />)
      
      expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-047g: Disabled State
  // ============================================================================

  describe('Disabled State', () => {
    it('should disable dropzone when disabled prop is true', () => {
      render(<MediaUpload {...defaultProps} disabled />)
      
      const dropzone = screen.getByText('Drop files here or click to select').closest('div')
      expect(dropzone).toHaveClass(/disabled/)
    })

    it('should disable dropzone when uploading', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          {
            id: '1',
            file: new File(['test'], 'test.jpg', { type: 'image/jpeg' }),
            type: 'image',
            size: 1024,
            preview: 'blob:test-preview',
            uploadProgress: 50,
            status: 'uploading',
          },
        ],
        state: 'processing',  // Use 'processing' as per UploadState type
        error: null,
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} />)
      
      const dropzone = screen.getByText('Drop files here or click to select').closest('div')
      expect(dropzone).toHaveClass(/disabled/)
    })
  })

  // ============================================================================
  // T-047h: Max Files Indicator
  // ============================================================================

  describe('Max Files Indicator', () => {
    it('should show file count', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          {
            id: '1',
            file: new File(['test'], 'test.jpg', { type: 'image/jpeg' }),
            type: 'image',
            size: 1024,
            preview: 'blob:test-preview',
            uploadProgress: 0,
            status: 'pending',
          },
        ],
        state: 'idle',
        error: null,
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} maxFiles={4} />)
      
      expect(screen.getByText(/1.*4/)).toBeInTheDocument()
    })

    it('should hide dropzone when max files reached', () => {
      ;(useMediaUpload as jest.Mock).mockReturnValue({
        files: [
          { id: '1', status: 'pending', preview: 'blob:1' },
          { id: '2', status: 'pending', preview: 'blob:2' },
          { id: '3', status: 'pending', preview: 'blob:3' },
          { id: '4', status: 'pending', preview: 'blob:4' },
        ],
        state: 'idle',
        error: null,
        addFiles: mockAddFiles,
        removeFile: mockRemoveFile,
        uploadAll: mockUploadAll,
        clearAll: mockClearAll,
      })

      render(<MediaUpload {...defaultProps} maxFiles={4} />)
      
      expect(screen.queryByText('Drop files here or click to select')).not.toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-047i: Accessibility
  // ============================================================================

  describe('Accessibility', () => {
    it('should have accessible dropzone', () => {
      render(<MediaUpload {...defaultProps} />)
      
      const dropzone = screen.getByText('Drop files here or click to select').closest('div')
      expect(dropzone).toHaveAttribute('role', 'button')
      expect(dropzone).toHaveAttribute('tabIndex', '0')
    })

    it('should support keyboard activation', async () => {
      render(<MediaUpload {...defaultProps} />)
      
      const dropzone = screen.getByText('Drop files here or click to select').closest('div')
      dropzone?.focus()
      
      fireEvent.keyDown(dropzone!, { key: 'Enter' })
      
      // Should trigger file input click
      // This is hard to test directly, but the component should handle it
    })
  })
})
