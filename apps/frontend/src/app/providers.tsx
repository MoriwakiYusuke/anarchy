'use client';

/**
 * ClientProviders Component
 * 
 * Wraps client-side providers for the application.
 * This is needed because layout.tsx is a Server Component.
 */

import React from 'react';
import type { PropsWithChildren } from 'react';
import { LocaleProvider } from '@/i18n';
import { MatrixBackground } from '@/components/MatrixBackground';

export function ClientProviders({ children }: PropsWithChildren) {
  return (
    <LocaleProvider>
      <MatrixBackground respectReducedMotion />
      {children}
    </LocaleProvider>
  );
}

export default ClientProviders;
