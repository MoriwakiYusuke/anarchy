import type { Metadata, Viewport } from 'next'
import './globals.css'
import { ClientProviders } from './providers'

export const viewport: Viewport = {
  width: 'device-width',
  initialScale: 1,
  // maximum-scale / user-scalable=no は付けない: ピンチズームを奪うのはアクセシビリティ上 NG。
  // iOS の入力フォーカス時の自動ズームは input の font-size を 16px 以上にして防ぐ。
  themeColor: '#000000',
}

export const metadata: Metadata = {
  title: 'Anarchy - Anonymous Decentralized SNS',
  description: 'Order without rulers. A truly free public space without centralized control.',
}

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>
        <ClientProviders>
          {children}
        </ClientProviders>
      </body>
    </html>
  )
}
