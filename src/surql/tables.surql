
DEFINE TABLE IF NOT EXISTS nsp_metadata SCHEMALESS;

DEFINE FIELD title_id ON nsp_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD title_name ON nsp_metadata TYPE option<string> PERMISSIONS FULL;