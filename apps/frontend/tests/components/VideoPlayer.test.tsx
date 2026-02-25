/**
 * VideoPlayer Component Tests
 * 
 * T-067: VideoPlayer component tests
 */

import React from 'react'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import VideoPlayer from '@/components/VideoPlayer'

// Mock video element methods
const mockPlay = jest.fn().mockResolvedValue(undefined)
const mockPause = jest.fn()
const mockLoad = jest.fn()

beforeAll(() => {
  // Mock HTMLVideoElement
  Object.defineProperty(HTMLVideoElement.prototype, 'play', {
    configurable: true,
    writable: true,
    value: mockPlay,
  })
  Object.defineProperty(HTMLVideoElement.prototype, 'pause', {
    configurable: true,
    writable: true,
    value: mockPause,
  })
  Object.defineProperty(HTMLVideoElement.prototype, 'load', {
    configurable: true,
    writable: true,
    value: mockLoad,
  })
})

beforeEach(() => {
  jest.clearAllMocks()
})

describe('VideoPlayer', () => {
  const testSrc = 'http://example.com/video.mp4'
  const testPoster = 'http://example.com/poster.jpg'

  it('renders video element with src', () => {
    render(<VideoPlayer src={testSrc} />)
    
    const video = screen.getByTestId('video-player')
    expect(video).toHaveAttribute('src', testSrc)
  })

  it('displays poster image when provided', () => {
    render(<VideoPlayer src={testSrc} poster={testPoster} />)
    
    const video = screen.getByTestId('video-player')
    expect(video).toHaveAttribute('poster', testPoster)
  })

  it('shows play button overlay initially', () => {
    render(<VideoPlayer src={testSrc} />)
    
    expect(screen.getByRole('button', { name: /play/i })).toBeInTheDocument()
  })

  it('calls play when play button clicked', async () => {
    render(<VideoPlayer src={testSrc} />)
    
    const playButton = screen.getByRole('button', { name: /play/i })
    fireEvent.click(playButton)
    
    await waitFor(() => {
      expect(mockPlay).toHaveBeenCalled()
    })
  })

  it('shows controls when autoPlay is true', () => {
    render(<VideoPlayer src={testSrc} autoPlay />)
    
    const video = screen.getByTestId('video-player')
    expect(video).toHaveAttribute('autoplay')
  })

  it('applies muted attribute when muted prop is true', () => {
    render(<VideoPlayer src={testSrc} muted />)
    
    const video = screen.getByTestId('video-player')
    expect(video).toHaveProperty('muted', true)
  })

  it('applies loop attribute when loop prop is true', () => {
    render(<VideoPlayer src={testSrc} loop />)
    
    const video = screen.getByTestId('video-player')
    expect(video).toHaveAttribute('loop')
  })

  it('displays duration when provided', () => {
    render(<VideoPlayer src={testSrc} duration={125} />)
    
    // 125 seconds = 2:05
    expect(screen.getByText('2:05')).toBeInTheDocument()
  })

  it('passes width and height to video element', () => {
    render(<VideoPlayer src={testSrc} width={640} height={360} />)
    
    const video = screen.getByTestId('video-player')
    expect(video).toHaveAttribute('width', '640')
    expect(video).toHaveAttribute('height', '360')
  })

  it('calls onPlay callback when video plays', async () => {
    const onPlay = jest.fn()
    render(<VideoPlayer src={testSrc} onPlay={onPlay} />)
    
    const video = screen.getByTestId('video-player')
    fireEvent.play(video)
    
    expect(onPlay).toHaveBeenCalled()
  })

  it('calls onPause callback when video pauses', () => {
    const onPause = jest.fn()
    render(<VideoPlayer src={testSrc} onPause={onPause} />)
    
    const video = screen.getByTestId('video-player')
    fireEvent.pause(video)
    
    expect(onPause).toHaveBeenCalled()
  })

  it('calls onEnded callback when video ends', () => {
    const onEnded = jest.fn()
    render(<VideoPlayer src={testSrc} onEnded={onEnded} />)
    
    const video = screen.getByTestId('video-player')
    fireEvent.ended(video)
    
    expect(onEnded).toHaveBeenCalled()
  })

  it('calls onError callback on video error', () => {
    const onError = jest.fn()
    render(<VideoPlayer src={testSrc} onError={onError} />)
    
    const video = screen.getByTestId('video-player')
    fireEvent.error(video)
    
    expect(onError).toHaveBeenCalled()
  })
})
