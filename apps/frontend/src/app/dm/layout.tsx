import type { ReactNode } from 'react';
import styles from './layout.module.css';

export default function DmLayout({ children }: { children: ReactNode }): JSX.Element {
  return <div className={styles.wrapper}>{children}</div>;
}
