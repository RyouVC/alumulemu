// game importer API for alu-panel
import { TitleMetadata } from "@/utils/title";

export async function importGameUltraNX(
  titleMetadata: TitleMetadata
): Promise<any> {
  const title_id = titleMetadata.titleId;

  // /admin/import/ultranx/{title_id} GET
  const response = await fetch(`/admin/import/ultranx/${title_id}`, {
    method: "GET",
  });

  // Always parse the response body, regardless of status code
  const data = await response.json();

  // If the request was not successful, but we received a response with a message
  if (!response.ok) {
    // If the response contains error details, use them
    if (data && data.message) {
      throw new Error(data.message);
    } else if (data && data.status === "error" && data.message) {
      throw new Error(data.message);
    } else {
      // Fall back to statusText if no detailed message is available
      throw new Error(`Error fetching game data: ${response.statusText}`);
    }
  }

  return data;
}

/**
 * Import a game by URL
 * @param url The URL to import from
 * @returns Promise with the import result
 */
export async function importGameURL(url: string): Promise<any> {
  // Ensure the URL is properly encoded
  const encodedURL = encodeURIComponent(url);

  // Use the auto-importer endpoint which will detect it's a URL and use the URL importer
  const response = await fetch(`/admin/import/auto/${encodedURL}`, {
    method: "GET",
  });

  // Always parse the response body, regardless of status code
  const data = await response.json();

  // If the request was not successful, but we received a response with a message
  if (!response.ok) {
    // If the response contains error details, use them
    if (data && data.message) {
      throw new Error(data.message);
    } else if (data && data.status === "error" && data.message) {
      throw new Error(data.message);
    } else {
      // Fall back to statusText if no detailed message is available
      throw new Error(`Error fetching game data: ${response.statusText}`);
    }
  }

  return data;
}
