
DEFINE TABLE IF NOT EXISTS user SCHEMALESS;
DEFINE ACCESS OVERWRITE user_access ON DATABASE TYPE RECORD
SIGNUP ( CREATE user SET username = $username, password = crypto::argon2::generate($password) )
SIGNIN ( SELECT * FROM user WHERE username = $username AND password = crypto::argon2::compare(password_hash, $password) )


-- 0.2.0 migration: Update existing users that don't have a scope field or have an empty array
UPDATE user 
SET scopes = ["viewer"] 
WHERE scopes = NONE OR scopes = [];