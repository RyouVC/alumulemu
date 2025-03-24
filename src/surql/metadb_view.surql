-- table that takes in nsp_metadata and fetches titles_US_en by looking WHERE titleId = $tid OR ids CONTAINS $tid

-- todo: figure out if this precomputes properly
DEFINE TABLE IF NOT EXISTS metaview_%LOCALE% TYPE NORMAL AS
    SELECT *
    FROM titles_%LOCALE%
    WHERE (
        SELECT title_id 
        FROM nsp_metadata
        WHERE 
            -- Standard matching with original title_id
            $parent.titleId = title_id 
            OR $parent.ids CONTAINS title_id

    );
