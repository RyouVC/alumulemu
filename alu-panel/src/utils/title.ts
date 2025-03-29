/**
 * Interface representing game/title metadata from Nintendo eShop
 */
export interface TitleMetadata {
  /** Unique numeric identifier */
  id: number;

  /** Title ID in Nintendo Switch format (16 character hex string) */
  titleId: string;

  /** Additional title IDs related to this title */
  titleIds: string[];

  /** URL to the banner image */
  bannerUrl: string | null;

  /** Game developer name */
  developer: string | null;

  /** URL to front box art */
  frontBoxArt: string | null;

  /** URL to the icon/thumbnail image */
  iconUrl: string | null;

  /** Short introduction or tagline */
  intro: string | null;

  /** Full game description */
  description: string;

  /** Game categories/genres */
  category: string[];

  /** Whether this is a demo version */
  isDemo: boolean;

  /** License key or similar (if applicable) */
  key: string | null;

  /** Supported languages (language codes) */
  languages: string[];

  /** Game title */
  name: string;

  /** Maximum number of players */
  numberOfPlayers: number;

  /** Age rating value */
  rating: number | null;

  ageRating: string | null;

  /** Content descriptors for rating */
  ratingContent: string[];

  /** Region information */
  region: string | null;

  /** Release date in YYYYMMDD format */
  releaseDate: string;

  /** Rights ID for the title */
  rightsId: string;

  /** URLs to game screenshots */
  screenshots: string[];

  /** Game file size in bytes */
  size: number;

  /** Game version */
  version: string | null;

  /** Game publisher name */
  publisher: string | null;
}

export class SearchQuery {
  query: string; // shortened to `q` in the API
  limit: number | null;

  constructor(query: string = "", limit: number | null = null) {
    this.query = query;
    this.limit = limit;
  }

  /**
   * Creates the URL query string for the search
   */
  toQueryString(): string {
    let params = [`q=${encodeURIComponent(this.query.trim())}`];
    console.log(params);
    if (this.limit !== null) {
      params.push(`limit=${this.limit}`);
    }

    return params.join("&");
  }
}

/**
 * Namespace for TitleMetadata-related operations
 */
export namespace TitleMetadata {
  /**
   * Represents the response structure for game search results
   */
  export interface SearchResponse {
    results: TitleMetadata[];
  }

  /**
   * Fetches all base games from the metaview
   * @returns Promise resolving to an array of TitleMetadata
   */
  export async function fetchBaseGames(): Promise<TitleMetadata[]> {
    const response = await fetch("/api/base_games");

    if (!response.ok) {
      throw new Error(
        `Failed to fetch games: ${response.status} ${response.statusText}`
      );
    }

    return await response.json();
  }

  /**
   * Searches for currently available games based on the provided search query
   * @param searchQuery The SearchQuery object containing search parameters
   * @returns Promise resolving to search results containing TitleMetadata
   */
  export async function searchAvailableGames(
    searchQuery: SearchQuery
  ): Promise<TitleMetadata[]> {
    const queryString = searchQuery.toQueryString();
    const response = await fetch(`/api/base_games/search?${queryString}`);

    if (!response.ok) {
      throw new Error(
        `Failed to search games: ${response.status} ${response.statusText}`
      );
    }

    const data = await response.json();

    // Handle both array response and object with results property
    if (Array.isArray(data)) {
      return data;
    } else if (data && data.error) {
      throw new Error(`Error from API: ${data.error}`);
    }

    return [];
  }

  /**
   * Search the entire title database for a game
   * @param searchQuery The SearchQuery object containing search parameters
   * @returns Promise resolving to search results containing TitleMetadata
   */
  export async function searchAllGames(
    searchQuery: SearchQuery
  ): Promise<TitleMetadata[]> {
    const queryString = searchQuery.toQueryString();
    const response = await fetch(`/api/search?${queryString}`);

    if (!response.ok) {
      throw new Error(
        `Failed to search title database: ${response.status} ${response.statusText}`
      );
    }

    const data = await response.json();

    if (Array.isArray(data)) {
      return data;
    } else if (data && data.error) {
      throw new Error(`Error from API: ${data.error}`);
    }

    return [];
  }

  /**
   * Fetches title metadata by title ID
   * @param titleId The title ID to fetch metadata for
   * @returns Promise resolving to the title metadata
   */
  export async function fetchMetaViewById(
    titleId: string
  ): Promise<TitleMetadata> {
    const response = await fetch(`/api/title_meta/${titleId}`);

    if (!response.ok) {
      throw new Error(`Failed to fetch metadata for title ${titleId}`);
    }

    return await response.json();
  }

  /**
   * Fetches download IDs associated with a title
   * @param titleId The title ID to fetch download IDs for
   * @returns Promise resolving to an array of download IDs
   */
  export async function fetchDownloadIds(titleId: string): Promise<string[]> {
    try {
      const response = await fetch(`/api/title_meta/${titleId}/download_ids`);

      if (!response.ok) {
        throw new Error(`Failed to fetch download IDs for title ${titleId}`);
      }

      return await response.json();
    } catch (error) {
      console.error("Error fetching download IDs:", error);
      return [];
    }
  }

  /**
   * Triggers a rescan of games
   * @param forceRescan Whether to force a complete rescan
   * @returns Promise resolving when the rescan is complete
   */
  export async function rescanGames(
    forceRescan: boolean = false
  ): Promise<void> {
    const url = "/admin/rescan" + (forceRescan ? "?rescan=true" : "");
    const response = await fetch(url, {
      method: "POST",
      credentials: "include",
      headers: {
        "Content-Type": "application/json",
      },
    });

    if (!response.ok) {
      throw new Error("Game rescan failed");
    }
  }
}
