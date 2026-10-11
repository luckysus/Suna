import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import StorageDataPanel from "./StorageDataPanel";
import { desktop } from "../../bridge/desktop";

vi.mock("../../bridge/desktop", () => ({
  isDesktopRuntime: () => true,
  desktop: {
    getStoragePaths: vi.fn(), getStorageHealth: vi.fn(), getDataDirectoryChange: vi.fn(),
    openFileDialog: vi.fn(), changeDataDirectory: vi.fn(), restartForDataDirectory: vi.fn(), cancelDataDirectoryChange: vi.fn(),
  },
}));

describe("data directory migration", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(desktop.getStoragePaths).mockResolvedValue({ app_data: "C:/old", documents: "C:/docs", local_data: "C:/local", sqlite_database: "C:/old/suna.sqlite3", knowledge_content: "C:/old/objects", models: "C:/old", cache: "C:/old/cache", logs: "C:/old/logs", temp: "C:/old/temp" });
    vi.mocked(desktop.getStorageHealth).mockResolvedValue({ database_ok: true, current_migration_version: 35, latest_migration_version: 35, database_size_bytes: 1024, reclaimable_bytes: 0, available_disk_bytes: 100000 });
    vi.mocked(desktop.getDataDirectoryChange).mockResolvedValue({ pending_directory: null, message: null, environment_override: false });
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  it("selects a directory, stages migration and offers an explicit restart", async () => {
    vi.mocked(desktop.openFileDialog).mockResolvedValue("D:/new");
    vi.mocked(desktop.changeDataDirectory).mockResolvedValue({ pending_directory: "D:/new", message: null, environment_override: false });
    render(<StorageDataPanel />);
    const change = await screen.findByRole("button", { name: "更改数据目录" });
    await waitFor(() => expect(change).toBeEnabled());
    fireEvent.click(change);
    expect(await screen.findByRole("button", { name: "重启并迁移" })).toBeEnabled();
    expect(desktop.changeDataDirectory).toHaveBeenCalledWith("D:/new");
    expect(desktop.openFileDialog).toHaveBeenCalledWith(expect.objectContaining({ directory: true }));
    expect(desktop.restartForDataDirectory).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "重启并迁移" }));
    await waitFor(() => expect(desktop.restartForDataDirectory).toHaveBeenCalledOnce());
  });

  it("does not stage migration when the picker or confirmation is cancelled", async () => {
    vi.mocked(desktop.openFileDialog).mockResolvedValue(null);
    render(<StorageDataPanel />);
    const change = await screen.findByRole("button", { name: "更改数据目录" });
    await waitFor(() => expect(change).toBeEnabled());
    fireEvent.click(change);
    await waitFor(() => expect(change).toBeEnabled());
    expect(desktop.changeDataDirectory).not.toHaveBeenCalled();
    vi.mocked(desktop.openFileDialog).mockResolvedValue("D:/new");
    vi.mocked(window.confirm).mockReturnValue(false);
    fireEvent.click(change);
    await waitFor(() => expect(window.confirm).toHaveBeenCalled());
    expect(desktop.changeDataDirectory).not.toHaveBeenCalled();
  });

  it("restores pending state after reopening settings and allows cancellation", async () => {
    vi.mocked(desktop.getDataDirectoryChange).mockResolvedValueOnce({ pending_directory: "D:/new", message: null, environment_override: false });
    render(<StorageDataPanel />);
    fireEvent.click(await screen.findByRole("button", { name: "取消迁移" }));
    await waitFor(() => expect(desktop.cancelDataDirectoryChange).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.queryByRole("button", { name: "重启并迁移" })).not.toBeInTheDocument());
  });

  it("disables switching under a fixed environment override", async () => {
    vi.mocked(desktop.getDataDirectoryChange).mockResolvedValue({ pending_directory: null, message: null, environment_override: true });
    render(<StorageDataPanel />);
    await waitFor(() => expect(screen.getByRole("button", { name: "更改数据目录" })).toBeDisabled());
    expect(screen.getByText(/移除该变量后可更改目录/)).toBeInTheDocument();
    expect(desktop.changeDataDirectory).not.toHaveBeenCalled();
  });
});
