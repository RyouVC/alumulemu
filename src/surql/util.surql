DEFINE ANALYZER IF NOT EXISTS alu_search TOKENIZERS blank,class,punct FILTERS lowercase,ascii,edgengram(2,10);

