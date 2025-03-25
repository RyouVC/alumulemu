DEFINE TABLE IF NOT EXISTS titles_%LOCALE% SCHEMALESS;
-- multiple title IDs (for DLCs, etc)
DEFINE FIELD ids ON titles_%LOCALE% FLEXIBLE TYPE option<array<string>> PERMISSIONS FULL;
-- NSU ID, actual unique identifier
DEFINE FIELD nsuId ON titles_%LOCALE% TYPE int PERMISSIONS FULL;
-- Title ID, optional unique identifier, used for actual released titles
DEFINE FIELD titleId ON titles_%LOCALE% FLEXIBLE TYPE option<string> PERMISSIONS FULL;


DEFINE INDEX IF NOT EXISTS idsIndex_%LOCALE% ON TABLE titles_%LOCALE% COLUMNS ids CONCURRENTLY;

DEFINE INDEX IF NOT EXISTS titleIdIndex_%LOCALE% ON TABLE titles_%LOCALE% COLUMNS titleId CONCURRENTLY;

DEFINE INDEX IF NOT EXISTS titleIdIndexAll_%LOCALE% ON TABLE titles_%LOCALE% COLUMNS titleId, ids CONCURRENTLY;

DEFINE INDEX IF NOT EXISTS
    search_index_%LOCALE%
    ON TABLE titles_%LOCALE%
    FIELDS name, publisher
    SEARCH ANALYZER alu_search BM25 CONCURRENTLY;

REBUILD INDEX idsIndex_%LOCALE% ON TABLE titles_%LOCALE%;
REBUILD INDEX titleIdIndex_%LOCALE% ON TABLE titles_%LOCALE%;
REBUILD INDEX search_index_%LOCALE% ON TABLE titles_%LOCALE%;
REBUILD INDEX titleIdIndexAll_%LOCALE% ON TABLE titles_%LOCALE%;