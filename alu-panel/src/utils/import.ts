// game importer API for alu-panel

// Define types for the API responses
export interface ApiResponse<T> {
  status: "success" | "error";
  message?: string;
  data?: T;
}

export interface ImporterInfo {
  id: string;
  display_name: string;
  description: string;
}

export interface ImportersResponse {
  importers: ImporterInfo[];
}

export interface ImportStartResponse {
  importer: string;
}

import { TitleMetadata } from "@/utils/title";

/**
 * Get a list of all available importers
 * @returns Promise with the list of importers
 */
export async function getImporters(): Promise<ImporterInfo[]> {
  const response = await fetch("/admin/import/list", {
    method: "GET",
  });

  const apiResponse = (await response.json()) as ApiResponse<ImportersResponse>;

  if (!response.ok || apiResponse.status === "error") {
    throw new Error(
      apiResponse.message || `Error fetching importers: ${response.statusText}`
    );
  }

  return apiResponse.data?.importers || [];
}

/**
 * Import a game using the UltraNX importer
 * @param titleMetadata The title metadata containing the title ID
 * @returns Promise with the import result
 */
export async function importGameUltraNX(
  titleMetadata: TitleMetadata,
  dl_type: string
): Promise<ImportStartResponse> {
  const payload = {
    title_id: titleMetadata.titleId,
    download_type: dl_type,
  };

  // Use the new JSON-based endpoint
  const response = await fetch(`/admin/import/ultranx`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(payload),
  });

  // Parse the response as an ApiResponse
  const apiResponse =
    (await response.json()) as ApiResponse<ImportStartResponse>;

  // Handle errors
  if (!response.ok || apiResponse.status === "error") {
    throw new Error(
      apiResponse.message || `Error importing game: ${response.statusText}`
    );
  }

  return apiResponse.data as ImportStartResponse;
}

/**
 * Import a game by URL
 * @param url The URL to import from
 * @returns Promise with the import result
 */
export async function importGameURL(url: string): Promise<ImportStartResponse> {
  const payload = { url };

  // Use the new JSON-based endpoint
  const response = await fetch(`/admin/import/url`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(payload),
  });

  // Parse the response as an ApiResponse
  const apiResponse =
    (await response.json()) as ApiResponse<ImportStartResponse>;

  // Handle errors
  if (!response.ok || apiResponse.status === "error") {
    throw new Error(
      apiResponse.message || `Error importing game: ${response.statusText}`
    );
  }

  return apiResponse.data as ImportStartResponse;
}

/**
 * Generic function to import a game using any registered importer
 * @param importerId The ID of the importer to use
 * @param payload The importer-specific JSON payload
 * @returns Promise with the import result
 */
export async function importGame(
  importerId: string,
  payload: any
): Promise<ImportStartResponse> {
  const response = await fetch(`/admin/import/${importerId}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(payload),
  });

  // Parse the response as an ApiResponse
  const apiResponse =
    (await response.json()) as ApiResponse<ImportStartResponse>;

  // Handle errors
  if (!response.ok || apiResponse.status === "error") {
    throw new Error(
      apiResponse.message || `Error importing game: ${response.statusText}`
    );
  }

  return apiResponse.data as ImportStartResponse;
}
