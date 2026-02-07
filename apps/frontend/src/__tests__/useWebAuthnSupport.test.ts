import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import {
  useWebAuthnSupport,
  isWebAuthnSupported,
  isPlatformAuthenticatorAvailable,
  isConditionalUIAvailable,
} from '../hooks/useWebAuthnSupport';
import { MockPublicKeyCredential } from './setup';

describe('useWebAuthnSupport', () => {
  beforeEach(() => {
    // Ensure mocks are properly set up before each test
    vi.clearAllMocks();
    
    // Reset to default mock behavior
    (MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = 
      vi.fn().mockResolvedValue(true);
    (MockPublicKeyCredential as any).isConditionalMediationAvailable = 
      vi.fn().mockResolvedValue(true);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('isWebAuthnSupported', () => {
    it('should return true when PublicKeyCredential exists', () => {
      expect(isWebAuthnSupported()).toBe(true);
    });

    it('should return false when PublicKeyCredential is undefined', () => {
      const pkc = (global as any).PublicKeyCredential;
      (global as any).PublicKeyCredential = undefined;
      
      expect(isWebAuthnSupported()).toBe(false);
      
      // Restore
      (global as any).PublicKeyCredential = pkc;
    });
  });

  describe('isPlatformAuthenticatorAvailable', () => {
    it('should return true when platform authenticator is available', async () => {
      (MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = 
        vi.fn().mockResolvedValue(true);
      
      const result = await isPlatformAuthenticatorAvailable();
      expect(result).toBe(true);
    });

    it('should return false when WebAuthn is not supported', async () => {
      const pkc = (global as any).PublicKeyCredential;
      (global as any).PublicKeyCredential = undefined;
      
      const result = await isPlatformAuthenticatorAvailable();
      expect(result).toBe(false);
      
      // Restore
      (global as any).PublicKeyCredential = pkc;
    });

    it('should return false when check throws', async () => {
      (MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = 
        vi.fn().mockRejectedValue(new Error('Not supported'));
      
      const result = await isPlatformAuthenticatorAvailable();
      expect(result).toBe(false);
    });
  });

  describe('isConditionalUIAvailable', () => {
    it('should return true when conditional mediation is available', async () => {
      (MockPublicKeyCredential as any).isConditionalMediationAvailable = 
        vi.fn().mockResolvedValue(true);
      
      const result = await isConditionalUIAvailable();
      expect(result).toBe(true);
    });

    it('should return false when not supported', async () => {
      (MockPublicKeyCredential as any).isConditionalMediationAvailable = undefined;
      
      const result = await isConditionalUIAvailable();
      expect(result).toBe(false);
    });

    it('should return false when WebAuthn is not supported', async () => {
      const pkc = (global as any).PublicKeyCredential;
      (global as any).PublicKeyCredential = undefined;
      
      const result = await isConditionalUIAvailable();
      expect(result).toBe(false);
      
      // Restore
      (global as any).PublicKeyCredential = pkc;
    });
  });

  describe('useWebAuthnSupport hook', () => {
    it('should complete checking after mount', async () => {
      const { result } = renderHook(() => useWebAuthnSupport());

      // After the effect runs, checking should be complete
      await waitFor(() => {
        expect(result.current.isChecking).toBe(false);
      });
      expect(result.current.isChecked).toBe(true);
    });

    it('should detect WebAuthn support', async () => {
      const { result } = renderHook(() => useWebAuthnSupport());

      await waitFor(() => {
        expect(result.current.isChecking).toBe(false);
      });

      expect(result.current.isSupported).toBe(true);
      expect(result.current.isChecked).toBe(true);
    });

    it('should detect platform authenticator', async () => {
      (MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = 
        vi.fn().mockResolvedValue(true);

      const { result } = renderHook(() => useWebAuthnSupport());

      await waitFor(() => {
        expect(result.current.isChecking).toBe(false);
      });

      expect(result.current.hasPlatformAuthenticator).toBe(true);
    });

    it('should handle no platform authenticator', async () => {
      (MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = 
        vi.fn().mockResolvedValue(false);

      const { result } = renderHook(() => useWebAuthnSupport());

      await waitFor(() => {
        expect(result.current.isChecking).toBe(false);
      });

      expect(result.current.hasPlatformAuthenticator).toBe(false);
    });

    it('should handle WebAuthn not supported', async () => {
      const pkc = (global as any).PublicKeyCredential;
      (global as any).PublicKeyCredential = undefined;

      const { result } = renderHook(() => useWebAuthnSupport());

      await waitFor(() => {
        expect(result.current.isChecking).toBe(false);
      });

      expect(result.current.isSupported).toBe(false);
      expect(result.current.hasPlatformAuthenticator).toBe(false);

      // Restore
      (global as any).PublicKeyCredential = pkc;
    });

    it('should allow recheck', async () => {
      (MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = 
        vi.fn().mockResolvedValue(false);

      const { result } = renderHook(() => useWebAuthnSupport());

      await waitFor(() => {
        expect(result.current.isChecking).toBe(false);
      });

      expect(result.current.hasPlatformAuthenticator).toBe(false);

      // Now update the mock and recheck
      (MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = 
        vi.fn().mockResolvedValue(true);

      await act(async () => {
        await result.current.recheck();
      });

      expect(result.current.hasPlatformAuthenticator).toBe(true);
    });

    it('should handle API throwing errors gracefully', async () => {
      // When isPlatformAuthenticatorAvailable throws, it returns false
      // The error is caught internally, so hasPlatformAuthenticator should be false not null
      (MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = 
        vi.fn().mockRejectedValue(new Error('Test error'));

      const { result } = renderHook(() => useWebAuthnSupport());

      await waitFor(() => {
        expect(result.current.isChecking).toBe(false);
      });

      // The isPlatformAuthenticatorAvailable function catches the error and returns false
      expect(result.current.hasPlatformAuthenticator).toBe(false);
      // Since the error is caught internally, no error is surfaced to the hook
      expect(result.current.error).toBe(null);
    });
  });
});
