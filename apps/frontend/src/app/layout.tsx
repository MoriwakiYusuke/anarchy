import type { Metadata } from 'next'
import './globals.css'

export const metadata: Metadata = {
  title: 'Anarchy - 匿名分散型SNS',
  description: '支配なき秩序。中央集権を排除した真の自由な広場。',
}

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="ja">
      <body>{children}</body>
    </html>
  )
}
