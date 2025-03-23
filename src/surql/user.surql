DEFINE ACCESS OVERWRITE user_access ON DATABASE TYPE RECORD
SIGNUP ( CREATE user SET username = $username, password = crypto::argon2::generate($password) )
SIGNIN ( SELECT * FROM user WHERE username = $username AND password = crypto::argon2::compare(password_hash, $password) )

