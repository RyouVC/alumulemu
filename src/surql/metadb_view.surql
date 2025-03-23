-- table that takes in nsp_metadata and fetches titles_US_en by looking WHERE titleId = $tid OR ids CONTAINS $tid



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
            
            -- For title_ids ending with '800', also match with '000' version
            OR (
                string::ends_with(title_id, '800')
                AND (
                    $parent.titleId = string::replace(title_id, '800$', '000')
                    OR $parent.ids CONTAINS string::replace(title_id, '800$', '000')
                )
            )
    );