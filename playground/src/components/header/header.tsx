import logo from "../../assets/logo.svg";
import { ThemeToggle } from "../theme-toggle/theme-toggle";
import styles from "./header.module.css";

export const Header = () => (
  <header className={styles.header}>
    <a className={styles.brand} href="https://nuten.keito.dev/">
      <img src={logo} alt="" width="28" height="28" />
      <span className={styles.name}>
        Nuten
        <span className={styles.title}>Playground</span>
      </span>
    </a>
    <nav className={styles.links}>
      <a className={styles.link} href="https://nuten.keito.dev/reference/settings">
        Docs
      </a>
      <a className={styles.link} href="https://github.com/mst-mkt/nuten">
        GitHub
      </a>
    </nav>
    <ThemeToggle />
  </header>
);
