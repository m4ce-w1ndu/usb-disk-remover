import {
  FluentProvider,
  webDarkTheme,
  webLightTheme,
  Text,
  Link,
  Divider,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { UsbPlug24Regular } from "@fluentui/react-icons";
import { useState, useEffect } from "react";

const useStyles = makeStyles({
  root: {
    height: "100%",
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    gap: "16px",
    padding: "32px",
    backgroundColor: "transparent",
    textAlign: "center",
  },
  icon: {
    color: tokens.colorBrandForeground1,
  },
  name: {
    marginTop: "4px",
  },
  meta: {
    color: tokens.colorNeutralForeground3,
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    alignItems: "center",
  },
  divider: {
    width: "100%",
    maxWidth: "240px",
  },
  footer: {
    color: tokens.colorNeutralForeground4,
    fontSize: tokens.fontSizeBase100,
  },
});

function useDarkMode() {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const [dark, setDark] = useState(mq.matches);
  useEffect(() => {
    const h = (e: MediaQueryListEvent) => setDark(e.matches);
    mq.addEventListener("change", h);
    return () => mq.removeEventListener("change", h);
  }, []);
  return dark;
}

export default function About() {
  const dark = useDarkMode();
  const styles = useStyles();

  return (
    <FluentProvider
      theme={dark ? webDarkTheme : webLightTheme}
      style={{ height: "100%", backgroundColor: "transparent" }}
    >
      <div className={styles.root}>
        <UsbPlug24Regular fontSize={48} className={styles.icon} />

        <div>
          <Text size={600} weight="semibold" block className={styles.name}>
            USB Disk Remover
          </Text>
          <Text size={200} className={styles.meta} block>
            Version 0.1.0
          </Text>
        </div>

        <div className={styles.divider}>
          <Divider />
        </div>

        <div className={styles.meta}>
          <Text size={200}>
            Safely eject USB and Firewire drives on Windows.
          </Text>
          <Text size={200}>
            A Rust port of{" "}
            <Link href="https://github.com/quickandeasysoftware/USB-Disk-Ejector" target="_blank">
              USB Disk Ejector
            </Link>{" "}
            by QuickAndEasySoftware.
          </Text>
        </div>

        <Text size={100} className={styles.footer}>
          MIT License · github.com/m4ce-w1ndu/usb-disk-remover
        </Text>
      </div>
    </FluentProvider>
  );
}
