/**
 * ConnectionState Types Tests
 * 
 * Tests for connection state type definitions and helper functions
 */

import {
  ConnectionState,
  ConnectionStatus,
  isConnected,
  isSyncing,
  canPerformOperations,
} from '@/types/connection'

describe('ConnectionState Types', () => {
  describe('ConnectionStatus type', () => {
    it('should accept valid status values', () => {
      const statuses: ConnectionStatus[] = [
        'initializing',
        'syncing', 
        'connected',
        'error',
      ]
      
      // Type check - this test passes if it compiles
      expect(statuses).toHaveLength(4)
    })
  })

  describe('ConnectionState interface', () => {
    it('should allow minimal state', () => {
      const state: ConnectionState = {
        status: 'initializing',
      }
      
      expect(state.status).toBe('initializing')
      expect(state.blockNumber).toBeUndefined()
      expect(state.errorMessage).toBeUndefined()
    })

    it('should allow full state for connected status', () => {
      const state: ConnectionState = {
        status: 'connected',
        blockNumber: 12345,
      }
      
      expect(state.status).toBe('connected')
      expect(state.blockNumber).toBe(12345)
    })

    it('should allow error message for error status', () => {
      const state: ConnectionState = {
        status: 'error',
        errorMessage: 'Connection failed',
      }
      
      expect(state.status).toBe('error')
      expect(state.errorMessage).toBe('Connection failed')
    })
  })

  describe('isConnected helper', () => {
    it('should return true when status is connected', () => {
      const state: ConnectionState = { status: 'connected', blockNumber: 100 }
      expect(isConnected(state)).toBe(true)
    })

    it('should return false when status is initializing', () => {
      const state: ConnectionState = { status: 'initializing' }
      expect(isConnected(state)).toBe(false)
    })

    it('should return false when status is syncing', () => {
      const state: ConnectionState = { status: 'syncing' }
      expect(isConnected(state)).toBe(false)
    })

    it('should return false when status is error', () => {
      const state: ConnectionState = { status: 'error', errorMessage: 'Failed' }
      expect(isConnected(state)).toBe(false)
    })
  })

  describe('isSyncing helper', () => {
    it('should return true when status is syncing', () => {
      const state: ConnectionState = { status: 'syncing' }
      expect(isSyncing(state)).toBe(true)
    })

    it('should return false when status is connected', () => {
      const state: ConnectionState = { status: 'connected', blockNumber: 100 }
      expect(isSyncing(state)).toBe(false)
    })

    it('should return false when status is initializing', () => {
      const state: ConnectionState = { status: 'initializing' }
      expect(isSyncing(state)).toBe(false)
    })

    it('should return false when status is error', () => {
      const state: ConnectionState = { status: 'error', errorMessage: 'Failed' }
      expect(isSyncing(state)).toBe(false)
    })
  })

  describe('canPerformOperations helper', () => {
    it('should return true when status is connected', () => {
      const state: ConnectionState = { status: 'connected', blockNumber: 100 }
      expect(canPerformOperations(state)).toBe(true)
    })

    it('should return false when status is syncing', () => {
      const state: ConnectionState = { status: 'syncing' }
      expect(canPerformOperations(state)).toBe(false)
    })

    it('should return false when status is initializing', () => {
      const state: ConnectionState = { status: 'initializing' }
      expect(canPerformOperations(state)).toBe(false)
    })

    it('should return false when status is error', () => {
      const state: ConnectionState = { status: 'error', errorMessage: 'Failed' }
      expect(canPerformOperations(state)).toBe(false)
    })
  })
})
