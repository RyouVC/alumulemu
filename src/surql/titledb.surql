DEFINE TABLE IF NOT EXISTS titles_%LOCALE% SCHEMALESS;
-- multiple title IDs (for DLCs, etc)
DEFINE FIELD ids ON titles_%LOCALE% FLEXIBLE TYPE option<string> PERMISSIONS FULL;
-- NSU ID, actual unique identifier
DEFINE FIELD nsuId ON titles_%LOCALE% TYPE int PERMISSIONS FULL;
-- Title ID, optional unique identifier, used for actual released titles
DEFINE FIELD titleId ON titles_%LOCALE% FLEXIBLE TYPE string PERMISSIONS FULL;

DEFINE INDEX titleIdIndex_%LOCALE% ON TABLE titles_%LOCALE% COLUMNS titleId, ids;
DEFINE INDEX titleSearchIndex_%LOCALE% ON TABLE titles_%LOCALE% FIELDS titleId SEARCH ANALYZER BM25 HIGHLIGHTS;
DEFINE INDEX multiTitleSearchIndex_%LOCALE% ON TABLE titles_%LOCALE% FIELDS ids SEARCH ANALYZER BM25 HIGHLIGHTS;

