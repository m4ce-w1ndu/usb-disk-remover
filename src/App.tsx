import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  FluentProvider,
  webDarkTheme,
  webLightTheme,
  DataGrid,
  DataGridHeader,
  DataGridHeaderCell,
  DataGridBody,
  DataGridRow,
  DataGridCell,
  createTableColumn,
  TableColumnDefinition,
  TableRowId,
  Button,
  Text,
  Spinner,
  Tooltip,
  makeStyles,
  tokens,
  mergeClasses,
} from "@fluentui/react-components";
import {
  UsbPlug24Regular,
  ArrowClockwise24Regular,
  ArrowEjectFilled,
} from "@fluentui/react-icons";

// ── Types ─────────────────────────────────────────────────────────────────────

type BusType = "Usb" | "Firewire" | "Unknown";

interface Drive {
  mount_point: string;
  label: string;
  vendor: string;
  product: string;
  bus_type: BusType;
  is_card_reader: boolean;
}

// ── Styles ────────────────────────────────────────────────────────────────────

const useStyles = makeStyles({
  provider: {
    height: "100%",
    display: "flex",
    flexDirection: "column",
    // Transparent so Mica shows through — Fluent tokens handle text colours
    backgroundColor: "transparent",
  },
  shell: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    padding: "12px 12px 8px 12px",
    gap: "8px",
  },
  toolbar: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    flexShrink: 0,
  },
  appTitle: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    flex: 1,
  },
  gridWrapper: {
    flex: 1,
    minHeight: 0,
    overflow: "auto",
    borderRadius: tokens.borderRadiusMedium,
    // Slightly opaque surface so rows are legible over Mica blur
    backgroundColor: tokens.colorNeutralBackground1,
    boxShadow: tokens.shadow4,
  },
  emptyState: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    height: "160px",
    gap: "8px",
    color: tokens.colorNeutralForeground3,
  },
  footer: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    flexShrink: 0,
    paddingTop: "2px",
  },
  status: {
    color: tokens.colorNeutralForeground3,
    fontSize: tokens.fontSizeBase200,
    transition: "color 0.2s",
  },
  statusError: {
    color: tokens.colorPaletteRedForeground1,
  },
  statusOk: {
    color: tokens.colorPaletteGreenForeground1,
  },
  headerCell: {
    fontWeight: tokens.fontWeightSemibold,
  },
  row: {
    cursor: "default",
  },
});

// ── Column definitions ────────────────────────────────────────────────────────

const columns: TableColumnDefinition<Drive>[] = [
  createTableColumn<Drive>({
    columnId: "mount_point",
    compare: (a, b) => a.mount_point.localeCompare(b.mount_point),
    renderHeaderCell: () => "Drive",
    renderCell: (item) => (
      <Text font="monospace" size={300}>
        {item.mount_point}
      </Text>
    ),
  }),
  createTableColumn<Drive>({
    columnId: "label",
    compare: (a, b) => a.label.localeCompare(b.label),
    renderHeaderCell: () => "Label",
    renderCell: (item) => (
      <Text>{item.label || <span style={{ opacity: 0.5 }}>No label</span>}</Text>
    ),
  }),
  createTableColumn<Drive>({
    columnId: "device",
    compare: (a, b) =>
      `${a.vendor} ${a.product}`.localeCompare(`${b.vendor} ${b.product}`),
    renderHeaderCell: () => "Device",
    renderCell: (item) => {
      const name = [item.vendor, item.product].filter(Boolean).join(" ") || "Unknown device";
      return <Text>{name}</Text>;
    },
  }),
];

const columnSizingOptions = {
  mount_point: { defaultWidth: 72, minWidth: 60 },
  label: { defaultWidth: 160, minWidth: 80 },
  device: { defaultWidth: 300, minWidth: 120 },
};

// ── Status message helper ─────────────────────────────────────────────────────

type StatusKind = "idle" | "ok" | "error";

interface Status {
  text: string;
  kind: StatusKind;
}

// ── App ───────────────────────────────────────────────────────────────────────

function useDarkMode() {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const [dark, setDark] = useState(mq.matches);
  useEffect(() => {
    const handler = (e: MediaQueryListEvent) => setDark(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);
  return dark;
}

export default function App() {
  const dark = useDarkMode();
  const styles = useStyles();

  const [drives, setDrives] = useState<Drive[]>([]);
  const [loading, setLoading] = useState(true);
  const [ejecting, setEjecting] = useState(false);
  const [selected, setSelected] = useState<Set<TableRowId>>(new Set());
  const [status, setStatus] = useState<Status>({ text: "", kind: "idle" });
  const statusTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const pushStatus = (text: string, kind: StatusKind, ttl?: number) => {
    if (statusTimer.current) clearTimeout(statusTimer.current);
    setStatus({ text, kind });
    if (ttl) {
      statusTimer.current = setTimeout(() => setStatus({ text: "", kind: "idle" }), ttl);
    }
  };

  const loadDrives = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<Drive[]>("list_drives");
      setDrives(result);
      setSelected(new Set());
      if (result.length === 0) {
        pushStatus("No removable drives detected.", "idle");
      } else {
        pushStatus(`${result.length} drive${result.length === 1 ? "" : "s"} detected.`, "idle");
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadDrives();
  }, [loadDrives]);

  const selectedMountPoint = selected.size > 0 ? (Array.from(selected)[0] as string) : null;
  const selectedDrive = drives.find(
    (d) => d.mount_point === selectedMountPoint
  );

  const handleEject = async () => {
    if (!selectedDrive || ejecting) return;
    setEjecting(true);
    try {
      await invoke("remove_drive", { mountPoint: selectedDrive.mount_point });
      pushStatus(`${selectedDrive.mount_point} safely removed.`, "ok", 4000);
      await loadDrives();
    } catch (err) {
      pushStatus(`Could not eject ${selectedDrive.mount_point}: ${err}`, "error", 6000);
    } finally {
      setEjecting(false);
    }
  };

  const handleRowDoubleClick = (drive: Drive) => {
    setSelected(new Set([drive.mount_point]));
    // Small delay so the selection visually registers before ejecting
    setTimeout(handleEject, 80);
  };

  const statusClass = mergeClasses(
    styles.status,
    status.kind === "error" && styles.statusError,
    status.kind === "ok" && styles.statusOk
  );

  return (
    <FluentProvider
      theme={dark ? webDarkTheme : webLightTheme}
      style={{ height: "100%", display: "flex", flexDirection: "column", backgroundColor: "transparent" }}
    >
      <div className={styles.shell}>
        {/* Toolbar */}
        <div className={styles.toolbar}>
          <div className={styles.appTitle}>
            <UsbPlug24Regular />
            <Text size={400} weight="semibold">
              USB Disk Remover
            </Text>
          </div>
          <Tooltip content="Refresh drive list" relationship="label">
            <Button
              icon={<ArrowClockwise24Regular />}
              appearance="subtle"
              onClick={loadDrives}
              disabled={loading || ejecting}
              aria-label="Refresh"
            />
          </Tooltip>
        </div>

        {/* Drive list */}
        <div className={styles.gridWrapper}>
          {loading ? (
            <div className={styles.emptyState}>
              <Spinner size="medium" label="Scanning drives…" />
            </div>
          ) : drives.length === 0 ? (
            <div className={styles.emptyState}>
              <UsbPlug24Regular fontSize={32} />
              <Text>No removable drives found.</Text>
            </div>
          ) : (
            <DataGrid
              items={drives}
              columns={columns}
              columnSizingOptions={columnSizingOptions}
              resizableColumns
              selectionMode="single"
              selectedItems={selected}
              onSelectionChange={(_, data) => setSelected(data.selectedItems)}
              getRowId={(item) => item.mount_point}
              focusMode="composite"
              sortable
            >
              <DataGridHeader>
                <DataGridRow>
                  {({ renderHeaderCell }) => (
                    <DataGridHeaderCell className={styles.headerCell}>
                      {renderHeaderCell()}
                    </DataGridHeaderCell>
                  )}
                </DataGridRow>
              </DataGridHeader>
              <DataGridBody<Drive>>
                {({ item, rowId }) => (
                  <DataGridRow<Drive>
                    key={rowId}
                    className={styles.row}
                    onDoubleClick={() => handleRowDoubleClick(item)}
                  >
                    {({ renderCell }) => (
                      <DataGridCell>{renderCell(item)}</DataGridCell>
                    )}
                  </DataGridRow>
                )}
              </DataGridBody>
            </DataGrid>
          )}
        </div>

        {/* Footer */}
        <div className={styles.footer}>
          <Text className={statusClass}>{status.text}</Text>
          <Button
            appearance="primary"
            icon={ejecting ? <Spinner size="tiny" /> : <ArrowEjectFilled />}
            disabled={!selectedDrive || ejecting}
            onClick={handleEject}
          >
            {ejecting ? "Removing…" : "Safely Remove"}
          </Button>
        </div>
      </div>
    </FluentProvider>
  );
}
