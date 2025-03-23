-- table that takes in nsp_metadata and fetches titles_US_en by looking WHERE titleId = $tid OR ids CONTAINS $tid


DEFINE TABLE IF NOT EXISTS metaview_%LOCALE% TYPE NORMAL AS
    SELECT *
    FROM nsp_metadata
    WHERE (
        SELECT *
        FROM titles_%LOCALE%
        WHERE 
            -- Standard matching with original title_id
            title_id = $parent.titleId
            OR ids CONTAINS $parent.titleId
    )
    GROUP BY title_id;