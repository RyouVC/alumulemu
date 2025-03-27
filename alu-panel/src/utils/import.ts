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

  if (!response.ok) {
    throw new Error(`Error fetching game data: ${response.statusText}`);
  }

  const data = await response.json();
  return data;
}
