-- table that takes in nsp_metadata and fetches titles_%LOCALE% by looking WHERE titleId = $tid OR ids CONTAINS $tid



-- this is very cursed, but it works.

-- The titledb dataset is massive though,
DEFINE TABLE IF NOT EXISTS metaview_%LOCALE% TYPE NORMAL AS
SELECT
    (SELECT * FROM titles_%LOCALE%
     WITH INDEX titleIdIndexAll_%LOCALE%, titleIdIndex_%LOCALE%, idsIndex_%LOCALE%
     WHERE
        -- Original condition
        titleId = $parent.title_id
        OR ids CONTAINS $parent.title_id
        
        -- New condition for 800 -> 000 replacement
        OR (
            $parent.title_id AND
            string::ends_with($parent.title_id, '800')
            AND (
                titleId = string::replace($parent.title_id, '800', '000')
                OR ids CONTAINS string::replace($parent.title_id, '800', '000')
            )
        )
    )[0] AS title,
    path,
    *
FROM nsp_metadata;

DEFINE FIELD title_name ON nsp_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD path ON nsp_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD title_id ON nsp_metadata TYPE string PERMISSIONS FULL;

DEFINE INDEX IF NOT EXISTS metaview_idx_%LOCALE%
    ON TABLE metaview_%LOCALE% 
    FIELDS title_name
    SEARCH ANALYZER alu_search BM25;