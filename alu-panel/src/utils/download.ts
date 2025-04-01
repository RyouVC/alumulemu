// Types for the Download API
import type { Ref } from "vue";

// Possible download statuses
export type DownloadStatus =
  | "Queued"
  | "Downloading"
  | "Paused"
  | "Completed"
  | "Cancelled"
  | "Failed";

// Failure status with error message
export interface FailedStatus {
  Failed: string;
}

// Download progress information
export interface Progress {
  total_size: number | null;
  downloaded: number;
  status: DownloadStatus | FailedStatus;
  file_path?: string | null;
}

// A single download queue item
export interface DownloadQueueItem {
  id?:
    | {
        tb: string;
        id: {
          String: string;
        };
      }
    | string;
  url: string;
  output_path: string;
  progress: Progress;
  created_at?: string;
}

// Combined structure returned by the API
export interface DownloadItemWithProgress {
  item: DownloadQueueItem;
  progress: Progress;
}

// Download statistics
export interface DownloadStats {
  total: number;
  queued: number;
  downloading: number;
  paused: number;
  completed: number;
  cancelled: number;
  failed: number;
}

// Download utilities
export const formatBytes = (bytes: number | null | undefined): string => {
  if (bytes === undefined || bytes === null) return "Unknown";

  const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
  if (bytes === 0) return "0 Bytes";

  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return parseFloat((bytes / Math.pow(1024, i)).toFixed(2)) + " " + sizes[i];
};

// Calculate percentage for progress bar
export const calculatePercentage = (progress: Progress): string => {
  if (!progress.total_size) return "0";
  return ((progress.downloaded / progress.total_size) * 100).toFixed(1);
};

// Helper function to get status string
export const getStatusString = (
  status: DownloadStatus | FailedStatus
): string => {
  if (typeof status === "string") {
    return status;
  } else if ("Failed" in status) {
    return `Failed: ${status.Failed}`;
  }
  return "Unknown";
};

// Download API functions
export const fetchDownloads = async (): Promise<
  Record<string, DownloadItemWithProgress>
> => {
  const response = await fetch("/api/downloads/");
  if (!response.ok) {
    throw new Error("Failed to fetch downloads");
  }
  return await response.json();
};

export const fetchStats = async (): Promise<DownloadStats> => {
  const response = await fetch("/api/downloads/stats");
  if (!response.ok) {
    throw new Error("Failed to fetch stats");
  }
  return await response.json();
};

export const cancelDownload = async (id: string): Promise<void> => {
  const response = await fetch(`/api/downloads/${id}/cancel`, {
    method: "GET",
  });
  if (!response.ok) {
    throw new Error("Failed to cancel download");
  }
};

export const cleanupDownloads = async (): Promise<{ count: number }> => {
  const response = await fetch(`/api/downloads/cleanup`, {
    method: "GET",
  });
  if (!response.ok) {
    throw new Error("Failed to clean up downloads");
  }
  return await response.json();
};
